use crate::auth::Auth;
use crate::creds::digest;
use crate::db::{cut, now, rows};
use crate::error::{Problem, Res, require};
use crate::mailbox::Note;
use crate::net::SYNC;
use crate::sync::{Head, head};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Deserialize;
use serde_json::{Value, json};

type Stored = (i64, String, bool, Vec<u8>, Option<Vec<u8>>, i64);

pub const PAGE_BYTES: i64 = 4 << 20;
pub const SYNC_FORMAT: i64 = 1;

const COLLECTIONS: [&str; 6] = ["repos", "library", "playlists", "settings", "device", "secrets"];

pub fn routes() -> Router {
    Router::new().route("/api/v1/sync/changes", get(pull).post(push).layer(DefaultBodyLimit::max(6 << 20)))
}

#[derive(Deserialize)]
struct Since {
    since: i64,
    limit: Option<i64>,
}

async fn pull(State(app): St, auth: Auth, Query(q): Query<Since>) -> Reply {
    let (user, limit) = (auth.device().map(|_| auth.user)?, q.limit.unwrap_or(500).clamp(1, 500));
    let sizes = "SELECT seq, COALESCE(LENGTH(payload), 0) FROM sync_records WHERE user_id = ?1 AND seq > ?2 \
                 ORDER BY seq LIMIT ?3";
    let sql = "SELECT collection, item, seq, epoch, device_id AS device, deleted, payload FROM sync_records \
               WHERE user_id = ?1 AND seq > ?2 AND seq <= ?3 ORDER BY seq";
    app.db
        .run(move |c| {
            let h = head(c, user)?;
            if q.since > h.seq || (q.since > 0 && q.since < h.min_seq) {
                let detail = "the server does not hold these changes: download again from since=0";
                return Err(Problem::new(StatusCode::GONE, "resync", detail));
            }
            let (last, more) = cut(c, sizes, params![user, q.since, limit + 1], q.since, limit, PAGE_BYTES)?;
            let mut changes = rows(c, sql, params![user, q.since, last])?;
            changes.iter_mut().for_each(|ch| ch["deleted"] = Value::Bool(ch["deleted"] == 1));
            let (seq, min_seq, epoch) = (h.seq, h.min_seq, h.epoch);
            Ok(Json(
                json!({"seq": seq, "min_seq": min_seq, "epoch": epoch, "more": more, "changes": changes}),
            ))
        })
        .await
}

#[derive(Deserialize)]
struct Change {
    collection: String,
    item: String,
    base: i64,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    payload: Option<String>,
}

#[derive(Deserialize)]
struct Push {
    epoch: i64,
    #[serde(default)]
    format: i64,
    changes: Vec<Change>,
}

fn decode(ch: &Change) -> Res<Option<Vec<u8>>> {
    require(
        COLLECTIONS.contains(&ch.collection.as_str()),
        format!("collection: one of {}", COLLECTIONS.join(", ")),
    )?;
    let item = ch.item.as_bytes();
    if ch.collection == "library" {
        let keyed = item.len() == 64 && item.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b));
        require(keyed, "item: library items are 64 hex digits of a keyed hash")?;
    } else {
        let plain = (1..=64).contains(&item.len())
            && item.iter().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(b));
        require(plain, "item: 1 to 64 letters, digits, - or _")?;
    }
    let payload = ch
        .payload
        .as_deref()
        .map(|p| B64.decode(p))
        .transpose()
        .map_err(|_| Problem::bad("payload: base64"))?;
    require(ch.deleted == payload.is_none(), "send a payload, or deleted: true without one")?;
    require(payload.as_ref().is_none_or(|p| p.len() <= 256 << 10), "payload: at most 256 KiB")?;
    Ok(payload)
}

fn sealed(payload: &[u8]) -> bool {
    payload.len() >= 41 + 256 && payload[0] == 1 && (payload.len() - 41).is_multiple_of(256)
}

const UPSERT: &str = "INSERT OR REPLACE INTO sync_records (user_id, collection, item, seq, epoch, \
                      device_id, deleted, hash, payload, updated_at) \
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

fn apply(
    tx: &Connection,
    h: &mut Head,
    user: i64,
    device: &str,
    ch: Change,
    payload: Option<Vec<u8>>,
) -> Res<Value> {
    require(
        !h.encrypted || payload.as_deref().is_none_or(sealed),
        "payload: expected an end-to-end encrypted record",
    )?;
    require(h.encrypted || ch.collection != "secrets", "the secrets collection needs end-to-end encryption")?;
    let hash = digest(payload.as_deref().unwrap_or_default());
    let sql = "SELECT seq, device_id, deleted, hash, payload, epoch FROM sync_records WHERE user_id = ?1 \
               AND collection = ?2 AND item = ?3";
    let read = |r: &Row| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?));
    let found: Option<Stored> = tx.query_row(sql, params![user, ch.collection, ch.item], read).optional()?;
    let mine = found.as_ref().is_some_and(|f| f.1 == device && f.2 == ch.deleted && f.3 == hash);
    Ok(match found {
        Some((seq, _, true, ..)) if ch.deleted => json!({"item": ch.item, "status": "ok", "seq": seq}),
        Some((seq, ..)) if seq != ch.base && mine => json!({"item": ch.item, "status": "ok", "seq": seq}),
        Some((seq, dev, deleted, _, old, epoch)) if seq != ch.base => {
            let payload = old.map(|o| B64.encode(o));
            let current =
                json!({"seq": seq, "epoch": epoch, "device": dev, "deleted": deleted, "payload": payload});
            json!({"item": ch.item, "status": "conflict", "current": current})
        }
        None if ch.base != 0 => json!({"item": ch.item, "status": "conflict", "current": null}),
        found => {
            let size = |p: Option<&Vec<u8>>| p.map_or(0, |p| p.len() as i64);
            h.used += size(payload.as_ref()) - size(found.as_ref().and_then(|f| f.4.as_ref()));
            h.seq += 1;
            let (seq, epoch, at) = (h.seq, h.epoch, now());
            tx.execute(
                UPSERT,
                params![user, ch.collection, ch.item, seq, epoch, device, ch.deleted, hash, payload, at],
            )?;
            json!({"item": ch.item, "status": "ok", "seq": seq})
        }
    })
}

async fn push(State(app): St, auth: Auth, Json(p): Json<Push>) -> Reply {
    let device = auth.device()?.to_string();
    app.limits.take(&SYNC, &device)?;
    require(p.changes.len() <= 500, "at most 500 changes per request")?;
    require(
        (0..=SYNC_FORMAT).contains(&p.format),
        format!("format: 0 to {SYNC_FORMAT}, the newest of this server"),
    )?;
    let decoded = p.changes.iter().map(decode).collect::<Res<Vec<_>>>()?;
    let (user, quota) = (auth.user, app.cfg.sync_quota as i64);
    let done = app.db.run(move |c| {
        let tx = c.transaction()?;
        let mut h = head(&tx, user)?;
        if p.epoch != h.epoch {
            return Err(Problem::conflict("epoch_mismatch", format!("the current key epoch is {}", h.epoch)));
        }
        if p.format < h.format {
            let detail =
                format!("this account is synced by a newer version (sync format {}): update", h.format);
            return Err(Problem::new(StatusCode::CONFLICT, "client_too_old", detail));
        }
        h.format = p.format;
        let changes = p.changes.into_iter().zip(decoded);
        let results = changes
            .map(|(ch, payload)| apply(&tx, &mut h, user, &device, ch, payload))
            .collect::<Res<Vec<_>>>()?;
        if h.used > quota {
            return Err(Problem::new(StatusCode::PAYLOAD_TOO_LARGE, "quota", "the sync quota is full"));
        }
        tx.execute(
            "UPDATE sync_heads SET seq = ?2, used_bytes = ?3, format = ?4 WHERE user_id = ?1",
            params![user, h.seq, h.used, h.format],
        )?;
        tx.commit()?;
        Ok((h.seq, results))
    });
    let (seq, results) = done.await?;
    app.hub.send(user, Note { event: "seq", device: None, value: seq });
    Ok(Json(json!({"seq": seq, "results": results})))
}
