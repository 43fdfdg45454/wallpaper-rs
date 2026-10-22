use crate::auth::{Auth, Role};
use crate::config::Publish;
use crate::creds::{hex, random};
use crate::db::{DAY, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::review::{approve, record};
use crate::validate::validate;
use crate::{App, Reply, Router, St};
use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use futures_util::{Stream, StreamExt};
use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fmt::Display;
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

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

async fn put(State(app): St, auth: Auth, Path(id): Path<String>, headers: HeaderMap, body: Body) -> Reply {
    auth.scope("upload")?;
    let (size, sha) =
        waiting(&app, auth.user, &id, "open").await?.ok_or_else(|| Problem::missing("open upload"))?;
    let length = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse::<i64>().ok());
    if length != Some(size) {
        return Err(Problem::new(
            StatusCode::LENGTH_REQUIRED,
            "length",
            "Content-Length must equal the declared size",
        ));
    }
    let _turn = app.pieces.enter(auth.user, &id)?;
    let staging = tmp(&app, &format!("{id}.part-{}", hex(&random::<8>())));
    let received = receive(body.into_data_stream(), &staging, size as u64).await;
    if !matches!(received, Ok((hash, total)) if hash == sha && total == size as u64) {
        let _ = fs::remove_file(&staging);
        let detail = "the body does not match the declared size and SHA-256";
        fail(&app, &id, detail).await;
        return Err(Problem::bad(detail));
    }
    fs::rename(&staging, tmp(&app, &id))?;
    let (key, sql) = (id.clone(), "UPDATE uploads SET state = 'received' WHERE id = ?1 AND state = 'open'");
    app.db.run(move |c| Ok(c.execute(sql, [key])?)).await?;
    status(&app, auth.user, id).await
}

async fn finalize(State(app): St, auth: Auth, Path(id): Path<String>) -> Reply {
    auth.scope("upload")?;
    finish(&app, &auth, id).await
}

async fn finish(app: &Arc<App>, auth: &Auth, id: String) -> Reply {
    let busy = || Problem::conflict("upload_state", "the upload is not complete or was already finalized");
    let (size, sha) = waiting(app, auth.user, &id, "received").await?.ok_or_else(busy)?;
    let thumb = tmp(app, &format!("{id}.jpg"));
    let checked = match validate(app, auth.user, &tmp(app, &id), &thumb).await {
        Ok(report) => record(app, auth.user, &id, &sha, size, report).await,
        Err(problem) => Err(problem),
    };
    let _ = fs::remove_file(&thumb);
    let policy = app.cfg.auto_publish;
    let trusted = policy == Publish::All || (policy == Publish::Trusted && auth.role >= Role::Trusted);
    match checked {
        Err(problem) => fail(app, &id, &problem.detail).await,
        Ok(case) if trusted && app.repo.ready() => {
            if let Err(problem) = approve(app, case).await {
                tracing::warn!(detail = %problem.detail, "automatic publication failed");
            }
        }
        Ok(_) => {}
    }
    status(app, auth.user, id).await
}

pub async fn status(app: &App, user: i64, id: String) -> Reply {
    let sql = "SELECT u.id, CASE v.state WHEN 'pending' THEN 'pending_review' ELSE COALESCE(v.state, \
               u.state) END AS state, u.errors, v.package_id AS package, v.version FROM uploads u \
               LEFT JOIN versions v ON v.id = u.version_id WHERE u.id = ?1 AND u.user_id = ?2";
    let found = app.db.run(move |c| rows(c, sql, params![id, user])).await?.pop();
    let mut found = found.ok_or_else(|| Problem::missing("upload"))?;
    let errors: Vec<Value> = found["errors"].as_str().map(|e| vec![e.into()]).unwrap_or_default();
    found["errors"] = errors.into();
    Ok(Json(found))
}

async fn show(State(app): St, auth: Auth, Path(id): Path<String>) -> Reply {
    auth.scope("upload")?;
    status(&app, auth.user, id).await
}

async fn cancel(State(app): St, auth: Auth, Path(id): Path<String>) -> Res<StatusCode> {
    auth.scope("upload")?;
    let (user, key, sql) =
        (auth.user, id.clone(), "DELETE FROM uploads WHERE id = ?1 AND user_id = ?2 AND state != 'done'");
    let gone = app.db.run(move |c| Ok(c.execute(sql, params![key, user])?)).await?;
    require(gone > 0, "only unfinished uploads can be cancelled")?;
    let _ = fs::remove_file(tmp(&app, &id));
    Ok(StatusCode::NO_CONTENT)
}

async fn reserve(app: &App, user: i64) -> Res<(String, u64)> {
    let (per_day, max) = (app.cfg.uploads_per_day as i64, app.cfg.package_max as i64);
    app.db
        .run(move |c| {
            let room = allowed(c, user, per_day)?.min(max);
            let (id, t) = (hex(&random::<16>()), now());
            let sql = "INSERT INTO uploads (id, user_id, size, sha256, state, created_at, expires_at) \
                       VALUES (?1, ?2, ?3, '', 'open', ?4, ?5)";
            c.execute(sql, params![id, user, room, t, t + DAY])?;
            Ok((id, room as u64))
        })
        .await
}

async fn package(app: &App, form: &mut Multipart, id: &str, room: u64) -> Res<()> {
    let late = |_| Problem::bad("the form took longer than 30 seconds to arrive");
    let wait = Duration::from_secs(30);
    for _ in 0..8 {
        let next = tokio::time::timeout(wait, form.next_field()).await.map_err(late)?;
        let Some(mut field) = next.map_err(|e| Problem::bad(e.body_text()))? else { break };
        if field.name() != Some("package") {
            let skip = async {
                let mut size = 0;
                while let Some(chunk) = field.chunk().await.map_err(|e| Problem::bad(e.body_text()))? {
                    size += chunk.len();
                    require(size <= 4096, "a form field is larger than 4 KiB")?;
                }
                Ok::<_, Problem>(())
            };
            tokio::time::timeout(wait, skip).await.map_err(late)??;
            continue;
        }
        let staging = tmp(app, &format!("{id}.part"));
        let (sha, size) = receive(field, &staging, room).await?;
        let (key, sql) = (
            id.to_string(),
            "UPDATE uploads SET size = ?2, sha256 = ?3, state = 'received' \
                                             WHERE id = ?1 AND state = 'open'",
        );
        let stored = app.db.run(move |c| {
            let blocked = "SELECT COUNT(*) FROM blocklist WHERE sha256 = ?1";
            if c.query_row(blocked, [&sha], |r| r.get::<_, i64>(0))? > 0 {
                return Err(Problem::forbidden("this file is on the blocklist"));
            }
            Ok(c.execute(sql, params![key, size as i64, sha])?)
        });
        require(stored.await? == 1, "the upload was cancelled")?;
        return Ok(fs::rename(&staging, tmp(app, id))?);
    }
    Err(Problem::bad("choose a .wpak file to upload"))
}

async fn web(State(app): St, auth: Auth, mut form: Multipart) -> Reply {
    auth.scope("upload")?;
    let (id, room) = reserve(&app, auth.user).await?;
    if let Err(problem) = package(&app, &mut form, &id, room).await {
        let _ = fs::remove_file(tmp(&app, &format!("{id}.part")));
        fail(&app, &id, &problem.detail).await;
        return Err(problem);
    }
    finish(&app, &auth, id).await
}
