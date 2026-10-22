use crate::auth::Auth;
use crate::error::{Input, Problem, Res, require};
use crate::mailbox::Note;
use crate::{Reply, Router, St, files};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Deserialize;
use serde_json::{Value, json};

const WARNING: &str = "plain-text sync: the operator of this server can read every record";

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/sync/state", get(state))
        .route("/api/v1/sync/setup", post(setup))
        .route("/api/v1/sync/epoch", post(epoch))
        .route("/api/v1/sync/reset", post(reset))
        .route("/api/v1/sync/escrow", get(escrow).put(store_escrow))
}

pub struct Head {
    pub seq: i64,
    pub min_seq: i64,
    pub epoch: i64,
    pub encrypted: bool,
    pub used: i64,
    pub key_id: Option<String>,
    pub rotate: bool,
    pub escrow: bool,
    pub files: i64,
}

pub fn head(c: &Connection, user: i64) -> Res<Head> {
    let sql = "SELECT seq, min_seq, epoch, encrypted, used_bytes, key_id, rotate, escrow IS NOT NULL, \
               (SELECT COALESCE(SUM(size), 0) FROM sync_blobs b WHERE b.user_id = ?1) FROM sync_heads \
               WHERE user_id = ?1";
    let read = |r: &Row| {
        let (seq, min_seq, epoch, encrypted, used) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
        let (key_id, rotate, escrow, files) = (r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?);
        Ok(Head { seq, min_seq, epoch, encrypted, used, key_id, rotate, escrow, files })
    };
    let unset = || Problem::new(StatusCode::NOT_FOUND, "sync_not_set_up", "set up sync first");
    c.query_row(sql, [user], read).optional()?.ok_or_else(unset)
}

fn describe(h: &Head, (quota, files_quota): (u64, u64)) -> Value {
    let (seq, min_seq, epoch, encrypted, used) = (h.seq, h.min_seq, h.epoch, h.encrypted, h.used);
    let warning = (!encrypted).then_some(WARNING);
    json!({
        "seq": seq, "min_seq": min_seq, "epoch": epoch, "encrypted": encrypted, "key_id": h.key_id,
        "rotate": h.rotate, "used_bytes": used, "quota_bytes": quota, "warning": warning,
        "escrow": h.escrow, "files_bytes": h.files, "files_quota_bytes": files_quota,
    })
}

fn key_id(id: &Option<String>) -> Res<()> {
    let hex =
        |k: &String| k.len() == 32 && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    require(id.as_ref().is_none_or(hex), "key_id: 32 lowercase hex digits")
}

async fn state(State(app): St, auth: Auth) -> Reply {
    let (user, quota) = (auth.device().map(|_| auth.user)?, (app.cfg.sync_quota, app.cfg.files_quota));
    Ok(Json(app.db.run(move |c| Ok(describe(&head(c, user)?, quota))).await?))
}

#[derive(Deserialize)]
struct Setup {
    #[serde(default = "yes")]
    encrypted: bool,
    #[serde(default)]
    confirm_plaintext: bool,
    #[serde(default)]
    key_id: Option<String>,
}

fn yes() -> bool {
    true
}

async fn setup(State(app): St, auth: Auth, Input(s): Input<Setup>) -> Res<impl IntoResponse> {
    let (user, quota) = (auth.device().map(|_| auth.user)?, (app.cfg.sync_quota, app.cfg.files_quota));
    require(
        s.encrypted || s.confirm_plaintext,
        "plain-text sync lets the server read every record: send confirm_plaintext: true",
    )?;
    key_id(&s.key_id)?;
    let state = app.db.run(move |c| {
        let sql =
            "INSERT INTO sync_heads (user_id, encrypted, key_id) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING";
        if c.execute(sql, params![user, s.encrypted, s.key_id])? == 0 {
            return Err(Problem::conflict(
                "already_set_up",
                "sync is already set up; reset it to change the mode",
            ));
        }
        Ok(describe(&head(c, user)?, quota))
    });
    Ok((StatusCode::CREATED, Json(state.await?)))
}

#[derive(Deserialize)]
struct Epoch {
    epoch: i64,
    #[serde(default)]
    key_id: Option<String>,
    #[serde(default)]
    done: bool,
}

const ROTATE: &str = "UPDATE sync_heads SET epoch = ?2, key_id = ?3, rotate = 0 WHERE user_id = ?1";
const ADOPT: &str = "UPDATE sync_heads SET key_id = ?2 WHERE user_id = ?1 AND key_id IS NULL";
const FINISH: &str = "DELETE FROM sync_records WHERE user_id = ?1 AND epoch < ?2";
const USED: &str = "UPDATE sync_heads SET used_bytes = (SELECT COALESCE(SUM(LENGTH(payload)), 0) \
                    FROM sync_records WHERE user_id = ?1) WHERE user_id = ?1";

async fn epoch(State(app): St, auth: Auth, Input(e): Input<Epoch>) -> Reply {
    let (user, quota) = (auth.device().map(|_| auth.user)?, (app.cfg.sync_quota, app.cfg.files_quota));
    key_id(&e.key_id)?;
    let state = app.db.run(move |c| {
        let tx = c.transaction()?;
        let h = head(&tx, user)?;
        let ok = match (e.epoch - h.epoch, e.done, e.key_id.as_deref()) {
            (1, false, Some(id)) => tx.execute(ROTATE, params![user, e.epoch, id])? > 0,
            (0, true, _) => {
                tx.execute(FINISH, params![user, e.epoch])?;
                tx.execute(USED, [user])? > 0
            }
            (0, false, Some(id)) if h.key_id.is_none() => tx.execute(ADOPT, params![user, id])? > 0,
            (0, false, Some(id)) => h.key_id.as_deref() == Some(id),
            _ => false,
        };
        if !ok {
            let detail = format!("the key epoch is {}; a new epoch is the current one plus one", h.epoch);
            return Err(Problem::conflict("epoch_mismatch", detail));
        }
        tx.commit()?;
        Ok(describe(&head(c, user)?, quota))
    });
    let state = state.await?;
    app.hub.send(user, Note { event: "epoch", device: None, value: e.epoch });
    Ok(Json(state))
}

#[derive(Deserialize)]
struct Reset {
    confirm: String,
}

async fn reset(State(app): St, auth: Auth, Input(r): Input<Reset>) -> Res<StatusCode> {
    auth.scope("sync")?;
    require(r.confirm == "reset", "confirm: send \"reset\" to delete every synced record")?;
    let (user, files) = (auth.user, files::dir(&app, auth.user));
    let cleared = app.db.run(move |c| {
        c.execute("DELETE FROM sync_records WHERE user_id = ?1", [user])?;
        c.execute("DELETE FROM sync_blobs WHERE user_id = ?1", [user])?;
        Ok(c.execute("DELETE FROM sync_heads WHERE user_id = ?1", [user])?)
    });
    cleared.await?;
    let _ = tokio::fs::remove_dir_all(&files).await;
    app.hub.send(user, Note { event: "epoch", device: None, value: -1 });
    Ok(StatusCode::NO_CONTENT)
}

async fn escrow(State(app): St, auth: Auth) -> Reply {
    let (user, sql) = (auth.device().map(|_| auth.user)?, "SELECT escrow FROM sync_heads WHERE user_id = ?1");
    let found =
        app.db.run(move |c| Ok(c.query_row(sql, [user], |r| r.get::<_, Option<Vec<u8>>>(0)).optional()?));
    Ok(Json(json!({"escrow": found.await?.flatten().map(|b| B64.encode(b))})))
}

#[derive(Deserialize)]
struct Escrow {
    escrow: Option<String>,
}

async fn store_escrow(State(app): St, auth: Auth, Input(e): Input<Escrow>) -> Res<StatusCode> {
    let user = auth.device().map(|_| auth.user)?;
    let blob = e.escrow.map(|t| B64.decode(t)).transpose().map_err(|_| Problem::bad("escrow: base64"))?;
    require(blob.as_ref().is_none_or(|b| b.len() <= 4096), "escrow: at most 4 KiB")?;
    let stored = app.db.run(move |c| {
        require(head(c, user)?.encrypted, "escrow needs end-to-end encryption")?;
        Ok(c.execute("UPDATE sync_heads SET escrow = ?2 WHERE user_id = ?1", params![user, blob])?)
    });
    stored.await?;
    Ok(StatusCode::NO_CONTENT)
}
