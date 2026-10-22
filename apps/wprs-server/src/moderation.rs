use crate::auth::{Auth, Role};
use crate::db::{audit, changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::net::{Client, REPORT};
use crate::review::{Decision, decide};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/mod/queue", get(queue))
        .route("/api/v1/mod/cases/{id}/decision", post(decision))
        .route("/api/v1/mod/blocklist", get(blocklist).post(block))
        .route("/api/v1/mod/blocklist/{sha256}", delete(unblock))
        .route("/api/v1/mod/audit", get(audit_log))
        .route("/api/v1/packages/{id}/reports", post(report))
}

pub fn open_cases(c: &Connection) -> Res<Vec<Value>> {
    let sql = "SELECT c.id, c.reason, c.detail, c.priority, c.created_at, v.package_id AS package, \
               v.version, v.title, v.state, v.sha256, u.handle AS uploader FROM cases c \
               JOIN versions v ON v.id = c.version_id LEFT JOIN users u ON u.id = v.uploaded_by \
               WHERE c.state = 'open' ORDER BY c.priority DESC, c.created_at LIMIT 200";
    rows(c, sql, [])
}

async fn queue(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Moderator)?;
    Ok(Json(json!({"items": app.db.run(|c| open_cases(c)).await?})))
}

async fn decision(State(app): St, auth: Auth, Path(id): Path<i64>, Input(d): Input<Decision>) -> Reply {
    auth.at_least(Role::Moderator)?;
    decide(&app, Some(auth.user), id, d).await.map(Json)
}

#[derive(Deserialize)]
struct Report {
    category: String,
    #[serde(default)]
    details: String,
}

async fn report(
    State(app): St,
    client: Client,
    Path(id): Path<String>,
    Input(r): Input<Report>,
) -> Res<impl IntoResponse> {
    app.limits.take(&REPORT, &client.key())?;
    let categories = ["copyright", "illegal", "malware", "mature", "spam", "other"];
    require(
        categories.contains(&r.category.as_str()),
        format!("category: one of {}", categories.join(", ")),
    )?;
    require(r.details.chars().count() <= 4000, "details: at most 4000 characters")?;
    let detail = format!("{}: {}", r.category, r.details.trim());
    let latest = "SELECT id FROM versions WHERE package_id = ?1 AND state = 'published' \
                  ORDER BY published_at DESC LIMIT 1";
    let insert = "INSERT INTO cases (version_id, reason, detail, priority, created_at) \
                  VALUES (?1, 'report', ?2, 1, ?3)";
    let case = app.db.run(move |c| {
        let version: Option<i64> = c.query_row(latest, [&id], |r| r.get(0)).optional()?;
        c.execute(insert, params![version.ok_or_else(|| Problem::missing("package"))?, detail, now()])?;
        Ok(c.last_insert_rowid())
    });
    Ok((StatusCode::CREATED, Json(json!({"case": case.await?}))))
}

#[derive(Deserialize)]
struct Block {
    sha256: String,
    reason: String,
}

async fn blocklist(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Moderator)?;
    app.db.items("SELECT sha256, reason, created_at FROM blocklist ORDER BY created_at DESC", vec![]).await
}

async fn block(State(app): St, auth: Auth, Input(b): Input<Block>) -> Res<StatusCode> {
    auth.at_least(Role::Moderator)?;
    let hex =
        b.sha256.len() == 64 && b.sha256.bytes().all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x));
    require(hex && !b.reason.trim().is_empty(), "expected a lowercase SHA-256 and a reason")?;
    let sql = "INSERT OR REPLACE INTO blocklist (sha256, reason, created_at) VALUES (?1, ?2, ?3)";
    let stored = app.db.run(move |c| {
        c.execute(sql, params![b.sha256, b.reason, now()])?;
        audit(c, Some(auth.user), "block", &b.sha256, &b.reason)
    });
    stored.await?;
    Ok(StatusCode::CREATED)
}

async fn unblock(State(app): St, auth: Auth, Path(sha): Path<String>) -> Res<StatusCode> {
    auth.at_least(Role::Moderator)?;
    let removed = app.db.run(move |c| {
        changed(c.execute("DELETE FROM blocklist WHERE sha256 = ?1", [&sha])?, "blocked hash")?;
        audit(c, Some(auth.user), "unblock", &sha, "")
    });
    removed.await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn audit_log(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    let sql = "SELECT a.at, u.handle AS actor, a.action, a.subject, a.detail FROM audit a \
               LEFT JOIN users u ON u.id = a.actor_id ORDER BY a.id DESC LIMIT 500";
    app.db.items(sql, vec![]).await
}
