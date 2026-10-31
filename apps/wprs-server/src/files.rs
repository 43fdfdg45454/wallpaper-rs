use crate::auth::Auth;
use crate::error::{Problem, Res, require};
use crate::{App, Router};
use axum::routing::get;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

mod data;
mod listing;

use data::{append, read, remove};
use listing::{list, open};

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
