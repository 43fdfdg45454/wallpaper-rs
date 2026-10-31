use crate::auth::Auth;
use crate::creds::{hex, random};
use crate::db::{DAY, now};
use crate::error::{Input, Problem, Res, require};
use crate::{App, Router, St};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use futures_util::{Stream, StreamExt};
use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fmt::Display;
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

mod finish;
mod package;

use finish::{cancel, finalize, finish, put, show};
use package::web;

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/uploads", post(open))
        .route("/api/v1/uploads/{id}", get(show).put(put).delete(cancel).layer(DefaultBodyLimit::disable()))
        .route("/api/v1/uploads/{id}/finalize", post(finalize))
        .route("/upload", post(web).layer(DefaultBodyLimit::disable()))
}

fn tmp(app: &App, name: &str) -> PathBuf {
    app.cfg.data.join("tmp").join(name)
}

#[derive(Deserialize)]
struct Open {
    size: u64,
    sha256: String,
}

async fn open(State(app): St, auth: Auth, Input(o): Input<Open>) -> Res<impl IntoResponse> {
    auth.scope("upload")?;
    let id = start(&app, &auth, o.size, o.sha256, "open").await?;
    let url = app.cfg.url(&format!("/api/v1/uploads/{id}"));
    Ok((StatusCode::CREATED, Json(json!({"id": id, "put_url": url, "expires_at": now() + DAY}))))
}

fn allowed(c: &Connection, user: i64, per_day: i64) -> Res<i64> {
    let t = now();
    let count = |sql: &str, values: &[&dyn ToSql]| c.query_row(sql, values, |r| r.get::<_, i64>(0));
    if count("SELECT COUNT(*) FROM strikes WHERE user_id = ?1 AND expires_at > ?2", params![user, t])? >= 3 {
        return Err(Problem::forbidden("uploads are suspended after three strikes"));
    }
    if count("SELECT COUNT(*) FROM uploads WHERE user_id = ?1 AND created_at > ?2", params![user, t - DAY])?
        >= per_day
    {
        return Err(Problem::busy(3600));
    }
    if count("SELECT COUNT(*) FROM uploads WHERE user_id = ?1 AND state = 'open'", params![user])? >= 2 {
        return Err(Problem::conflict("too_many_uploads", "finish or cancel an open upload first"));
    }
    let free = "SELECT (SELECT quota_bytes FROM users WHERE id = ?1) - COALESCE(SUM(size), 0) FROM versions \
                WHERE uploaded_by = ?1 AND state != 'rejected'";
    let free = count(free, params![user])?;
    require(free > 0, "your storage quota is full")?;
    Ok(free)
}

async fn start(app: &App, auth: &Auth, size: u64, sha256: String, state: &'static str) -> Res<String> {
    let (max, per_day, user) = (app.cfg.package_max, app.cfg.uploads_per_day as i64, auth.user);
    require((1..=max).contains(&size), format!("size: 1 to {max} bytes"))?;
    let hex64 =
        sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    require(hex64, "sha256: 64 lowercase hex digits")?;
    app.db
        .run(move |c| {
            let blocked = "SELECT COUNT(*) FROM blocklist WHERE sha256 = ?1";
            if c.query_row(blocked, [&sha256], |r| r.get::<_, i64>(0))? > 0 {
                return Err(Problem::forbidden("this file is on the blocklist"));
            }
            require(
                allowed(c, user, per_day)? >= size as i64,
                "the upload does not fit in your storage quota",
            )?;
            let (id, t) = (hex(&random::<16>()), now());
            let sql = "INSERT INTO uploads (id, user_id, size, sha256, state, created_at, expires_at) \
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
            c.execute(sql, params![id, user, size as i64, sha256, state, t, t + DAY])?;
            Ok(id)
        })
        .await
}

async fn receive<E: Display>(
    mut stream: impl Stream<Item = Result<Bytes, E>> + Unpin,
    path: &FsPath,
    max: u64,
) -> Res<(String, u64)> {
    let mut file = tokio::fs::File::create(path).await?;
    let (mut hasher, mut total) = (Sha256::new(), 0u64);
    let copy = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Problem::bad(format!("upload interrupted: {e}")))?;
            total += chunk.len() as u64;
            require(total <= max, "the body is larger than declared or allowed")?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
        }
        Ok::<_, Problem>(file.sync_all().await?)
    };
    let late = |_| Problem::bad("the upload took longer than 10 minutes");
    tokio::time::timeout(Duration::from_secs(600), copy).await.map_err(late)??;
    Ok((hex(&hasher.finalize()), total))
}

async fn waiting(app: &App, user: i64, id: &str, state: &'static str) -> Res<Option<(i64, String)>> {
    let (id, sql) =
        (id.to_string(), "SELECT size, sha256 FROM uploads WHERE id = ?1 AND user_id = ?2 AND state = ?3");
    app.db
        .run(move |c| {
            Ok(c.query_row(sql, params![id, user, state], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
        })
        .await
}

async fn fail(app: &App, id: &str, detail: &str) {
    let _ = fs::remove_file(tmp(app, id));
    let (id, detail) = (id.to_string(), detail.to_string());
    let sql = "UPDATE uploads SET state = 'failed', errors = ?2 WHERE id = ?1 AND state != 'done'";
    let _ = app.db.run(move |c| Ok(c.execute(sql, params![id, detail])?)).await;
}
