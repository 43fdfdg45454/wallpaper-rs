use crate::db::now;
use crate::error::Problem;
use crate::net::{API, client_ip, key};
use crate::pages::local;
use crate::web::layout;
use crate::{
    App, St, account, admin, catalog, device, legal, mailbox, me, moderation, pages, records, repo, sync,
    upload, web,
};
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, MatchedPath, Request, State};
use axum::http::header::{self, HeaderName, HeaderValue};
use axum::http::{Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use maud::html;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CSP: &str = "default-src 'none'; img-src 'self'; style-src 'self'; form-action 'self'; \
                   frame-ancestors 'none'; base-uri 'none'";

pub fn router(app: Arc<App>) -> Router {
    let mut modules: Vec<fn() -> crate::Router> =
        vec![account::routes, me::routes, device::routes, catalog::routes];
    modules.extend([
        upload::routes,
        moderation::routes,
        admin::routes,
        sync::routes,
        records::routes,
        mailbox::routes,
    ]);
    modules.extend([web::routes, pages::routes, legal::routes]);
    #[cfg(feature = "oidc")]
    modules.push(crate::oidc::routes);
    let inner = modules
        .into_iter()
        .fold(Router::new(), |router, routes| router.merge(routes()))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(ready))
        .route("/.well-known/wprs-server", get(discovery))
        .route("/repo/{*path}", get(repo::serve))
        .fallback(|| async { Problem::missing("page") })
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .layer(DefaultBodyLimit::max(1 << 20))
        .with_state(app);
    Router::new().fallback_service(inner).layer(middleware::from_fn(rewrite))
}

async fn rewrite(mut req: Request, next: Next) -> Response {
    let wanted =
        req.uri().query().into_iter().flat_map(|q| q.split('&')).find_map(|p| p.strip_prefix("_method="));
    let method = match wanted {
        Some("DELETE") => Some(Method::DELETE),
        Some("PATCH") => Some(Method::PATCH),
        _ => None,
    };
    if let Some(method) = method.filter(|_| req.method() == Method::POST) {
        *req.method_mut() = method;
    }
    next.run(req).await
}

fn csrf_ok(req: &Request) -> bool {
    let h = req.headers();
    if req.method().is_safe()
        || h.get(header::AUTHORIZATION).is_some_and(|v| v.as_bytes().starts_with(b"Bearer "))
    {
        return true;
    }
    if h.get("sec-fetch-site").is_some_and(|s| s != "same-origin" && s != "none") {
        return false;
    }
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok());
    h.get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|o| o.split_once("://").map(|(_, rest)| rest) == host)
}

async fn guard(State(app): St, req: Request, next: Next) -> Response {
    let started = Instant::now();
    let (method, path) = (req.method().clone(), req.uri().path().to_string());
    let route = req.extensions().get::<MatchedPath>().map_or("-", MatchedPath::as_str).to_string();
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let ip = client_ip(&app.cfg, req.headers(), peer);
    let (agent, kind) = {
        let text = |name: HeaderName| req.headers().get(name).and_then(|v| v.to_str().ok()).map(String::from);
        (text(header::USER_AGENT), text(header::CONTENT_TYPE).unwrap_or_default())
    };
    let form =
        kind.starts_with("application/x-www-form-urlencoded") || kind.starts_with("multipart/form-data");
    let back = req
        .uri()
        .query()
        .into_iter()
        .flat_map(|q| q.split('&'))
        .find_map(|p| p.strip_prefix("back="))
        .map(String::from);
    let signed =
        req.headers().contains_key(header::AUTHORIZATION) || req.headers().contains_key(header::COOKIE);
    let long = path.starts_with("/api/v1/uploads/") || path.ends_with("/decision") || path == "/upload";
    let long = long || path == "/api/v1/sync/events";
    let probe = path == "/healthz" || path == "/readyz";
    let mut response = match app.limits.take(&API, &key(ip, 32, 64)) {
        Err(problem) if !probe => problem.into_response(),
        _ if !csrf_ok(&req) => Problem::forbidden("cross-site request refused").into_response(),
        _ if long => next.run(req).await,
        _ => tokio::time::timeout(Duration::from_secs(30), next.run(req)).await.unwrap_or_else(|_| {
            Problem::new(StatusCode::SERVICE_UNAVAILABLE, "timeout", "the request took too long")
                .into_response()
        }),
    };
    let api = ["/api/", "/repo/", "/.well-known/"].iter().any(|p| path.starts_with(p));
    if form || !api {
        response = html(&app, response, form, back.as_deref()).await;
    }
    secure(&app, &method, &path, signed, &mut response);
    let status = response.status().as_u16();
    let ms = started.elapsed().as_millis() as u64;
    match probe {
        true => tracing::debug!(%method, route, status, ms, "request"),
        false => tracing::info!(%method, route, status, ms, "request"),
    }
    app.clients.write(ip, agent.as_deref(), method.as_str(), &route, status);
    response
}

async fn html(app: &App, response: Response, form: bool, back: Option<&str>) -> Response {
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let problem = kind.starts_with("application/problem+json");
    let done = form
        && response.status().is_success()
        && (kind.starts_with("application/json") || response.status() == StatusCode::NO_CONTENT);
    if !problem && !done {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let value: Value =
        serde_json::from_slice(&to_bytes(body, 1 << 20).await.unwrap_or_default()).unwrap_or_default();
    let back = local(back.unwrap_or("/"));
    let title = if problem { parts.status.canonical_reason().unwrap_or("Error") } else { "Done" };
    let shown = |v: &Value| v.as_str().map_or_else(|| v.to_string(), String::from);
    let body = html! {
        @if problem { p { (shown(&value["detail"])) } } @else {
            dl { @for (k, v) in value.as_object().into_iter().flatten() {
                dt { (k.replace('_', " ")) } dd { code { (shown(v)) } }
            } }
        }
        p { a href=(back) { "Continue" } }
    };
    parts.status = if problem { parts.status } else { StatusCode::OK };
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    Response::from_parts(parts, Body::from(layout(app, None, title, body).into_string()))
}

fn secure(app: &App, method: &Method, path: &str, signed: bool, response: &mut Response) {
    let open = method == Method::GET
        && ["/repo/", "/media/", "/api/v1/catalog", "/api/v1/packages/"].iter().any(|p| path.starts_with(p))
        && !path.ends_with("/reports");
    let page =
        response.headers().get(header::CONTENT_TYPE).is_some_and(|v| v.as_bytes().starts_with(b"text/html"));
    let h = response.headers_mut();
    let mut set =
        |name: &'static str, value: &'static str| drop(h.insert(name, HeaderValue::from_static(value)));
    set("x-content-type-options", "nosniff");
    set("referrer-policy", "no-referrer");
    set("cross-origin-opener-policy", "same-origin");
    set("cross-origin-resource-policy", if open { "cross-origin" } else { "same-site" });
    set("permissions-policy", "camera=(), microphone=(), geolocation=()");
    if page {
        set("content-security-policy", CSP);
    }
    if open {
        set("access-control-allow-origin", "*");
    }
    if app.cfg.secure() {
        set("strict-transport-security", "max-age=63072000; includeSubDomains");
    }
    if signed && !h.contains_key(header::CACHE_CONTROL) {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
}

async fn ready(State(app): St) -> Response {
    let database = app.db.run(|c| Ok(c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?)).await.is_ok();
    let repo = app.repo.info(&app.cfg);
    let expires = repo
        .as_ref()
        .and_then(|r| r["entry_expires"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()));
    let fresh = repo.is_none() || expires.is_some_and(|t| t.timestamp_millis() > now());
    let status = if database && fresh { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(json!({"database": database, "repository": repo.is_some(), "index_fresh": fresh})))
        .into_response()
}

async fn discovery(State(app): St) -> Json<Value> {
    let c = &app.cfg;
    let repo = app
        .repo
        .info(c)
        .map(|r| json!({"url": r["url"], "repo_id": r["repo_id"], "entry_expires": r["entry_expires"]}));
    let registration = format!("{:?}", c.registration).to_lowercase();
    let sync = json!({
        "end_to_end_default": true, "plaintext_requires_confirmation": true,
        "record_max_bytes": 262144, "quota_bytes": c.sync_quota,
    });
    let legal = json!({
        "notice": c.url("/legal/notice"), "terms": c.url("/legal/terms"),
        "privacy": c.url("/legal/privacy"), "contact": c.contact,
    });
    Json(json!({
        "name": c.name, "version": env!("CARGO_PKG_VERSION"), "api": ["v1"], "registration": registration,
        "oidc": c.oidc.is_some(), "sync": sync, "repository": repo, "legal": legal,
        "uploads": {"max_bytes": c.package_max, "per_day": c.uploads_per_day, "licenses": c.licenses},
    }))
}
