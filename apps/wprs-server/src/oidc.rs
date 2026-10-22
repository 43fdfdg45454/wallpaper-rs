use crate::account::insert_user;
use crate::auth::Auth;
use crate::creds::{self, hex, random};
use crate::db::one;
use crate::error::{Problem, Res};
use crate::net::{Client, LOGIN};
use crate::pages::local;
use crate::{App, Router, St};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AsyncHttpClient, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, TokenResponse,
};
use rusqlite::params;
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

type Sso = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

struct Flow {
    state: String,
    nonce: Nonce,
    verifier: PkceCodeVerifier,
    next: String,
    link: Option<i64>,
    at: Instant,
}

#[derive(Default)]
pub struct Oidc {
    flows: Mutex<HashMap<String, Flow>>,
}

pub struct Http(reqwest::Client);

impl<'c> AsyncHttpClient<'c> for Http {
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, io::Error>> + Send + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let request = reqwest::Request::try_from(request).map_err(io::Error::other)?;
            let response = self.0.execute(request).await.map_err(io::Error::other)?;
            let mut out = axum::http::Response::builder().status(response.status());
            for (name, value) in response.headers() {
                out = out.header(name, value);
            }
            out.body(response.bytes().await.map_err(io::Error::other)?.to_vec()).map_err(io::Error::other)
        })
    }
}

fn http() -> Res<Http> {
    let client = reqwest::Client::builder()
        .user_agent("wprs-server")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10));
    Ok(Http(client.build().map_err(Problem::internal)?))
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/auth/oidc/start", get(start))
        .route("/api/v1/auth/oidc/callback", get(callback))
}

async fn client(app: &App) -> Res<Sso> {
    let cfg = app.cfg.oidc.as_ref().ok_or_else(|| Problem::missing("single sign-on"))?;
    let issuer = IssuerUrl::new(cfg.issuer.clone()).map_err(Problem::internal)?;
    let found = CoreProviderMetadata::discover_async(issuer, &http()?).await;
    let metadata = found.map_err(|e| Problem::internal(format!("discovery failed: {e}")))?;
    let (id, secret) =
        (ClientId::new(cfg.client_id.clone()), Some(ClientSecret::new(cfg.client_secret.clone())));
    let redirect = RedirectUrl::new(app.cfg.url("/api/v1/auth/oidc/callback")).map_err(Problem::internal)?;
    Ok(CoreClient::from_provider_metadata(metadata, id, secret).set_redirect_uri(redirect))
}

#[derive(Deserialize)]
struct Start {
    #[serde(default)]
    next: String,
    #[serde(default)]
    link: bool,
}

async fn start(State(app): St, auth: Option<Auth>, peer: Client, Query(s): Query<Start>) -> Res<Response> {
    app.limits.take(&LOGIN, &peer.key())?;
    let link = if s.link { Some(auth.ok_or_else(Problem::unauthorized)?.user) } else { None };
    let sso = client(&app).await?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = sso
        .authorize_url(CoreAuthenticationFlow::AuthorizationCode, CsrfToken::new_random, Nonce::new_random)
        .set_pkce_challenge(challenge)
        .url();
    let id = hex(&random::<16>());
    let flow = Flow {
        state: state.secret().clone(),
        nonce,
        verifier,
        next: local(&s.next).to_string(),
        link,
        at: Instant::now(),
    };
    let mut flows = app.oidc.flows.lock().unwrap_or_else(PoisonError::into_inner);
    flows.retain(|_, f| f.at.elapsed() < Duration::from_secs(600));
    flows.insert(id.clone(), flow);
    let secure = if app.cfg.secure() { "; Secure" } else { "" };
    let cookie =
        format!("wprs-oidc={id}; Path=/api/v1/auth/oidc; HttpOnly; SameSite=Lax; Max-Age=600{secure}");
    Ok(([(header::SET_COOKIE, cookie)], Redirect::to(url.as_str())).into_response())
}

#[derive(Deserialize)]
struct Callback {
    code: String,
    state: String,
}

async fn callback(State(app): St, headers: HeaderMap, Query(c): Query<Callback>) -> Res<Response> {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let id = cookie.split(';').find_map(|p| p.trim().strip_prefix("wprs-oidc=")).unwrap_or_default();
    let flow = app.oidc.flows.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
    let flow = flow
        .filter(|f| f.state == c.state && f.at.elapsed() < Duration::from_secs(600))
        .ok_or_else(|| Problem::bad("the sign-in attempt expired, start again"))?;
    let sso = client(&app).await?;
    let request = sso.exchange_code(AuthorizationCode::new(c.code)).map_err(Problem::internal)?;
    let tokens = request
        .set_pkce_verifier(flow.verifier)
        .request_async(&http()?)
        .await
        .map_err(|e| Problem::forbidden(format!("the identity provider refused: {e}")))?;
    let token =
        tokens.id_token().ok_or_else(|| Problem::forbidden("the identity provider sent no ID token"))?;
    let claims = token
        .claims(&sso.id_token_verifier(), &flow.nonce)
        .map_err(|e| Problem::forbidden(format!("invalid ID token: {e}")))?;
    let (issuer, subject) = (claims.issuer().to_string(), claims.subject().to_string());
    let wanted = claims.preferred_username().map(|u| u.to_string()).unwrap_or_default();
    let (provision, quota) = (app.cfg.oidc.as_ref().is_some_and(|o| o.auto_provision), app.cfg.user_quota);
    let (link, next) = (flow.link, flow.next);
    let user = app.db.run(move |c| {
        let bind = "INSERT OR REPLACE INTO identities (issuer, subject, user_id) VALUES (?1, ?2, ?3)";
        let known = "SELECT user_id FROM identities WHERE issuer = ?1 AND subject = ?2";
        let bound: Option<i64> = one(c, known, params![issuer, subject])?;
        if link.is_some_and(|l| bound.is_some_and(|b| b != l)) {
            return Err(Problem::conflict("identity_taken", "this identity is linked to another account"));
        }
        if let Some(user) = link.or(bound) {
            c.execute(bind, params![issuer, subject, user])?;
            return Ok(user);
        }
        if !provision {
            return Err(Problem::forbidden(
                "no account is linked to this identity: sign in and link it from your account",
            ));
        }
        let base: String = wanted
            .to_lowercase()
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || "-_".contains(*ch))
            .collect();
        let base = if base.len() < 3 { "user".to_string() } else { base.chars().take(24).collect() };
        let taken = |h: &String| {
            one::<bool>(c, "SELECT 1 FROM users WHERE handle = ?1", [h]).is_ok_and(|t| t.is_some())
        };
        let handle =
            (0..100).map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") }).find(|h| !taken(h));
        let handle = handle.ok_or_else(|| Problem::conflict("handle_taken", "no free handle"))?;
        let user = insert_user(c, &handle, None, "user", quota)?;
        c.execute(bind, params![issuer, subject, user])?;
        Ok(user)
    });
    let user = user.await?;
    let session = app.db.run(move |c| creds::session(c, user)).await?;
    Ok(([(header::SET_COOKIE, creds::cookie(&app.cfg, &session, 30 * 86400))], Redirect::to(&next))
        .into_response())
}
