use crate::St;
use crate::db::now;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

pub async fn ready(State(app): St) -> Response {
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

pub async fn discovery(State(app): St) -> Json<Value> {
    let c = &app.cfg;
    let repo = app
        .repo
        .info(c)
        .map(|r| json!({"url": r["url"], "repo_id": r["repo_id"], "entry_expires": r["entry_expires"]}));
    let registration = format!("{:?}", c.registration).to_lowercase();
    let sync = json!({
        "end_to_end_default": true, "plaintext_requires_confirmation": true,
        "record_max_bytes": 262144, "quota_bytes": c.sync_quota, "files_quota_bytes": c.files_quota,
    });
    let legal = json!({
        "notice": c.url("/legal/notice"), "terms": c.url("/legal/terms"),
        "privacy": c.url("/legal/privacy"), "contact": c.contact,
    });
    Json(json!({
        "name": c.name, "version": crate::VERSION, "api": ["v1"], "registration": registration,
        "oidc": c.oidc.is_some(), "sync": sync, "repository": repo, "legal": legal,
        "uploads": {"max_bytes": c.package_max, "per_day": c.uploads_per_day, "licenses": c.licenses},
    }))
}
