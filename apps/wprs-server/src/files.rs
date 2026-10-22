use crate::auth::Auth;
use crate::db::{changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::sync::head;
use crate::{App, Reply, Router, St};
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use futures_util::StreamExt;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

pub const CHUNK: i64 = 8 << 20;
const PAGE: usize = 1000;

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/sync/blobs", get(list))
        .route("/api/v1/sync/blobs/{id}", get(read).post(open).put(append).delete(remove))
}

#[derive(Default)]
pub struct Pieces(Mutex<HashSet<(i64, String)>>);

pub struct Turn<'a>(&'a Pieces, (i64, String));

impl Pieces {
    pub fn enter(&self, user: i64, id: &str) -> Res<Turn<'_>> {
        let mut busy = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (user, id.to_string());
        if busy.contains(&key) || busy.iter().filter(|(u, _)| *u == user).count() >= 4 {
            return Err(Problem::busy(5));
        }
        busy.insert(key.clone());
        Ok(Turn(self, key))
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.0.0.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.1);
    }
}

pub fn dir(app: &App, user: i64) -> PathBuf {
    app.cfg.data.join("private").join(user.to_string())
}

fn device(auth: &Auth, id: &str) -> Res<i64> {
    let hex = id.len() == 64 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    require(hex || id.is_empty(), "id: 64 lowercase hex digits")?;
    auth.device().map(|_| auth.user)
}

fn found(c: &Connection, user: i64, id: &str) -> Res<(i64, i64)> {
    let sql = "SELECT size, received FROM sync_blobs WHERE user_id = ?1 AND id = ?2";
    let found = c.query_row(sql, params![user, id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    found.ok_or_else(|| Problem::missing("file"))
}

fn used(c: &Connection, user: i64) -> Res<i64> {
    let sql = "SELECT COALESCE(SUM(size), 0) FROM sync_blobs WHERE user_id = ?1";
    Ok(c.query_row(sql, [user], |r| r.get(0))?)
}

async fn row(app: &App, user: i64, id: &str) -> Res<(i64, i64)> {
    let id = id.to_string();
    app.db.run(move |c| found(c, user, &id)).await
}

async fn held(app: &App, user: i64, id: &str, received: i64) -> Res<i64> {
    let path = dir(app, user).join(id);
    let actual = tokio::fs::metadata(&path).await.map_or(0, |m| m.len() as i64);
    if actual >= received {
        return Ok(received);
    }
    let (id, sql) = (id.to_string(), "UPDATE sync_blobs SET received = ?3 WHERE user_id = ?1 AND id = ?2");
    app.db.run(move |c| Ok(c.execute(sql, params![user, id, actual])?)).await?;
    Ok(actual)
}

#[derive(Deserialize)]
struct After {
    #[serde(default)]
    after: String,
}

async fn list(State(app): St, auth: Auth, Query(a): Query<After>) -> Reply {
    let (user, quota) = (device(&auth, "")?, app.cfg.files_quota);
    let sql = "SELECT id, size, received, created_at FROM sync_blobs WHERE user_id = ?1 AND id > ?2 \
               ORDER BY id LIMIT ?3";
    app.db
        .run(move |c| {
            let mut items = rows(c, sql, params![user, a.after, PAGE as i64 + 1])?;
            let more = items.len() > PAGE;
            items.truncate(PAGE);
            let used = used(c, user)?;
            Ok(Json(json!({"items": items, "more": more, "used_bytes": used, "quota_bytes": quota})))
        })
        .await
}

#[derive(Deserialize)]
struct Open {
    size: i64,
}

async fn open(State(app): St, auth: Auth, Path(id): Path<String>, Input(o): Input<Open>) -> Reply {
    let (user, quota) = (device(&auth, &id)?, app.cfg.files_quota as i64);
    require(o.size > 0, "size: at least 1 byte")?;
    let key = id.clone();
    let (size, received) = app
        .db
        .run(move |c| {
            require(head(c, user)?.encrypted, "private files need end-to-end encryption")?;
            let tx = c.transaction()?;
            let stored = match found(&tx, user, &key) {
                Ok((size, _)) if size != o.size => {
                    return Err(Problem::conflict("size", "a file with this id has another size"));
                }
                Ok(stored) => stored,
                Err(_) if used(&tx, user)? + o.size > quota => {
                    let detail = "the quota of private files is full";
                    return Err(Problem::new(StatusCode::PAYLOAD_TOO_LARGE, "quota", detail));
                }
                Err(_) => {
                    let sql =
                        "INSERT INTO sync_blobs (user_id, id, size, created_at) VALUES (?1, ?2, ?3, ?4)";
                    tx.execute(sql, params![user, key, o.size, now()])?;
                    (o.size, 0)
                }
            };
            tx.commit()?;
            Ok(stored)
        })
        .await?;
    let received = held(&app, user, &id, received).await?;
    Ok(Json(json!({"id": id, "size": size, "received": received})))
}

#[derive(Deserialize)]
struct At {
    #[serde(default)]
    offset: i64,
    length: Option<i64>,
}

async fn append(
    State(app): St,
    auth: Auth,
    Path(id): Path<String>,
    Query(at): Query<At>,
    headers: HeaderMap,
    body: Body,
) -> Reply {
    let user = device(&auth, &id)?;
    let length = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse::<i64>().ok());
    let length = length.filter(|n| (1..=CHUNK).contains(n)).ok_or_else(|| {
        Problem::new(StatusCode::LENGTH_REQUIRED, "length", "send a Content-Length of 1 byte to 8 MiB")
    })?;
    let _turn = app.pieces.enter(user, &id)?;
    let (size, received) = row(&app, user, &id).await?;
    let received = held(&app, user, &id, received).await?;
    if at.offset != received {
        return Err(Problem::conflict("offset", format!("the server holds {received} bytes")));
    }
    require(received + length <= size, "the data goes past the declared size")?;
    let folder = dir(&app, user);
    tokio::fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).await?;
    let mut file =
        tokio::fs::OpenOptions::new().create(true).write(true).truncate(false).open(folder.join(&id)).await?;
    file.set_len(received as u64).await?;
    file.seek(SeekFrom::Start(received as u64)).await?;
    let copy = async {
        let (mut stream, mut total) = (body.into_data_stream(), 0);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Problem::bad(format!("upload interrupted: {e}")))?;
            total += chunk.len() as i64;
            require(total <= length, "the body is longer than its Content-Length")?;
            file.write_all(&chunk).await?;
        }
        require(total == length, "the body is shorter than its Content-Length")?;
        Ok::<_, Problem>(file.sync_data().await?)
    };
    let late = |_| Problem::bad("the piece took longer than 2 minutes");
    tokio::time::timeout(Duration::from_secs(120), copy).await.map_err(late)??;
    let (key, done) = (id.clone(), received + length);
    let sql = "UPDATE sync_blobs SET received = ?3 WHERE user_id = ?1 AND id = ?2 AND received = ?4";
    let moved = app.db.run(move |c| Ok(c.execute(sql, params![user, key, done, received])?)).await?;
    if moved == 0 {
        return Err(Problem::conflict("offset", "the file changed while this piece arrived"));
    }
    Ok(Json(json!({"id": id, "size": size, "received": done})))
}

async fn read(
    State(app): St,
    auth: Auth,
    Path(id): Path<String>,
    Query(at): Query<At>,
) -> Res<impl IntoResponse> {
    let user = device(&auth, &id)?;
    let (size, received) = row(&app, user, &id).await?;
    if received < size || held(&app, user, &id, received).await? < size {
        return Err(Problem::conflict("incomplete", "the file is still being uploaded"));
    }
    require((0..=size).contains(&at.offset), "offset: inside the file")?;
    let length = at.length.unwrap_or(CHUNK).clamp(0, CHUNK).min(size - at.offset);
    let path = dir(&app, user).join(&id);
    let bytes = tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path)?;
        file.seek(SeekFrom::Start(at.offset as u64))?;
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes).map(|()| bytes)
    });
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes.await??))
}

async fn remove(State(app): St, auth: Auth, Path(id): Path<String>) -> Res<StatusCode> {
    let user = device(&auth, &id)?;
    let path = dir(&app, user).join(&id);
    let sql = "DELETE FROM sync_blobs WHERE user_id = ?1 AND id = ?2";
    app.db.run(move |c| changed(c.execute(sql, params![user, id])?, "file")).await?;
    let _ = tokio::fs::remove_file(&path).await;
    Ok(StatusCode::NO_CONTENT)
}
