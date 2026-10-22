use crate::auth::Auth;
use crate::error::{Input, Problem, Res, require};
use crate::mailbox::Note;
use crate::{Reply, Router, St};
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
}

pub fn head(c: &Connection, user: i64) -> Res<Head> {
    let sql = "SELECT seq, min_seq, epoch, encrypted, used_bytes FROM sync_heads WHERE user_id = ?1";
    let read = |r: &Row| {
        Ok(Head {
            seq: r.get(0)?,
            min_seq: r.get(1)?,
            epoch: r.get(2)?,
            encrypted: r.get(3)?,
            used: r.get(4)?,
        })
    };
    let unset = || Problem::new(StatusCode::NOT_FOUND, "sync_not_set_up", "set up sync first");
    c.query_row(sql, [user], read).optional()?.ok_or_else(unset)
}

fn describe(h: &Head, quota: u64) -> Value {
    let (seq, min_seq, epoch, encrypted, used) = (h.seq, h.min_seq, h.epoch, h.encrypted, h.used);
    let warning = (!encrypted).then_some(WARNING);
    json!({
        "seq": seq, "min_seq": min_seq, "epoch": epoch, "encrypted": encrypted,
        "used_bytes": used, "quota_bytes": quota, "warning": warning,
    })
}

async fn state(State(app): St, auth: Auth) -> Reply {
    let (user, quota) = (auth.device().map(|_| auth.user)?, app.cfg.sync_quota);
    Ok(Json(app.db.run(move |c| Ok(describe(&head(c, user)?, quota))).await?))
}

#[derive(Deserialize)]
struct Setup {
    #[serde(default = "yes")]
    encrypted: bool,
    #[serde(default)]
    confirm_plaintext: bool,
}

fn yes() -> bool {
    true
}

async fn setup(State(app): St, auth: Auth, Input(s): Input<Setup>) -> Res<impl IntoResponse> {
    let (user, quota) = (auth.device().map(|_| auth.user)?, app.cfg.sync_quota);
    require(
        s.encrypted || s.confirm_plaintext,
        "plain-text sync lets the server read every record: send confirm_plaintext: true",
    )?;
    let state = app.db.run(move |c| {
        let sql = "INSERT INTO sync_heads (user_id, encrypted) VALUES (?1, ?2) ON CONFLICT DO NOTHING";
        if c.execute(sql, params![user, s.encrypted])? == 0 {
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
}

async fn epoch(State(app): St, auth: Auth, Input(e): Input<Epoch>) -> Reply {
    let (user, quota) = (auth.device().map(|_| auth.user)?, app.cfg.sync_quota);
    let state = app.db.run(move |c| {
        let sql = "UPDATE sync_heads SET epoch = ?2 WHERE user_id = ?1 AND epoch = ?2 - 1";
        if c.execute(sql, params![user, e.epoch])? == 0 {
            return Err(Problem::conflict(
                "epoch_mismatch",
                "the new epoch must be the current one plus one",
            ));
        }
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
    let user = auth.user;
    let cleared = app.db.run(move |c| {
        c.execute("DELETE FROM sync_records WHERE user_id = ?1", [user])?;
        Ok(c.execute("DELETE FROM sync_heads WHERE user_id = ?1", [user])?)
    });
    cleared.await?;
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
