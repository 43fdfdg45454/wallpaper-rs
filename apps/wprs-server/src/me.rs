use crate::account::profile;
use crate::auth::{Auth, Role, SCOPES};
use crate::creds::{self, digest};
use crate::db::{DAY, changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::mailbox::Note;
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/me/export", get(export))
        .route("/api/v1/me/packages", get(packages))
        .route("/api/v1/me/tokens", get(|s: St, a: Auth| mine(s, a, "tokens")).post(new_token))
        .route("/api/v1/me/devices", get(|s: St, a: Auth| mine(s, a, "devices")))
        .route("/api/v1/me/author-keys", get(|s: St, a: Auth| mine(s, a, "keys")).post(new_key))
        .route("/api/v1/me/{what}/{id}", delete(remove))
}

const TOKENS: &str = "SELECT id, name, scopes, device_id, created_at, used_at, expires_at FROM tokens \
                      WHERE user_id = ?1";
const DEVICES: &str = "SELECT id, 'Device ' || number AS name, platform, enc_key, sig_key, created_at, \
                       seen_at FROM devices WHERE user_id = ?1 ORDER BY number";
const KEYS: &str = "SELECT fingerprint, public_key, created_at FROM author_keys WHERE user_id = ?1";
const UPLOADS: &str = "SELECT id, state, errors, created_at FROM uploads WHERE user_id = ?1 \
                       AND state != 'done' ORDER BY created_at DESC LIMIT 50";
const VERSIONS: &str = "SELECT v.package_id AS package, v.version, v.title, v.state, v.created_at, \
                        v.published_at, c.decision, c.statement FROM versions v \
                        LEFT JOIN cases c ON c.id = (SELECT id FROM cases WHERE version_id = v.id \
                        AND decided_at IS NOT NULL ORDER BY decided_at DESC LIMIT 1) \
                        WHERE v.uploaded_by = ?1 ORDER BY v.created_at DESC";

pub fn listing(c: &Connection, user: i64, what: &str) -> Res<Vec<Value>> {
    let sql = match what {
        "tokens" => TOKENS,
        "devices" => DEVICES,
        "keys" => KEYS,
        "uploads" => UPLOADS,
        _ => VERSIONS,
    };
    rows(c, sql, [user])
}

async fn mine(State(app): St, auth: Auth, what: &'static str) -> Reply {
    Ok(Json(json!({"items": app.db.run(move |c| listing(c, auth.user, what)).await?})))
}

async fn packages(State(app): St, auth: Auth) -> Reply {
    auth.scope("upload")?;
    let lists =
        app.db.run(move |c| Ok((listing(c, auth.user, "versions")?, listing(c, auth.user, "uploads")?)));
    let (versions, uploads) = lists.await?;
    Ok(Json(json!({"items": versions, "uploads": uploads})))
}

async fn export(State(app): St, auth: Auth) -> Reply {
    auth.browser()?;
    let user = auth.user;
    let head = "SELECT seq, min_seq, epoch, encrypted, escrow, used_bytes FROM sync_heads WHERE user_id = ?1";
    let records = "SELECT collection, item, seq, epoch, device_id AS device, deleted, payload, updated_at \
                   FROM sync_records WHERE user_id = ?1 ORDER BY seq";
    let files =
        "SELECT id, size, received, created_at FROM sync_blobs WHERE user_id = ?1 ORDER BY created_at";
    let out = app.db.run(move |c| {
        let (sync, records) = (rows(c, head, [user])?, rows(c, records, [user])?);
        let files = rows(c, files, [user])?;
        let mut out = json!({"profile": profile(c, user)?, "sync": sync, "records": records, "files": files});
        for what in ["tokens", "devices", "keys", "versions", "uploads"] {
            out[what] = listing(c, user, what)?.into();
        }
        Ok(out)
    });
    Ok(Json(out.await?))
}

#[derive(Deserialize)]
struct NewToken {
    name: String,
    scopes: String,
    #[serde(default)]
    days: Option<i64>,
}

async fn new_token(State(app): St, auth: Auth, Input(t): Input<NewToken>) -> Res<impl IntoResponse> {
    auth.browser()?;
    let scopes: Vec<&str> = t.scopes.split([' ', ',']).filter(|s| !s.is_empty()).collect();
    let known = !scopes.is_empty() && scopes.iter().all(|s| SCOPES.contains(s));
    require(known, format!("scopes: some of {}", SCOPES.join(" ")))?;
    require(!t.name.trim().is_empty() && t.name.chars().count() <= 64, "name: 1 to 64 characters")?;
    let (moderate, administer) = (scopes.contains(&"mod"), scopes.contains(&"admin"));
    require(!moderate || auth.role >= Role::Moderator, "scopes: mod needs the moderator role")?;
    require(!administer || auth.role >= Role::Admin, "scopes: admin needs the admin role")?;
    let (days, most) =
        if moderate || administer { (t.days.unwrap_or(30), 90) } else { (t.days.unwrap_or(365), 365) };
    require((1..=most).contains(&days), "days: 1 to 365, at most 90 for mod and admin scopes")?;
    let (token, scopes, expires) = (creds::token('p'), scopes.join(" "), now() + days * DAY);
    let (hash, shown, name) = (digest(&token), scopes.clone(), t.name.trim().to_string());
    let sql = "INSERT INTO tokens (user_id, name, hash, scopes, created_at, expires_at) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
    let id = app.db.run(move |c| {
        c.execute(sql, params![auth.user, name, hash, scopes, now(), expires])?;
        Ok(c.last_insert_rowid())
    });
    let id = id.await?;
    Ok((StatusCode::CREATED, Json(json!({"id": id, "token": token, "scopes": shown, "expires_at": expires}))))
}

#[derive(Deserialize)]
struct NewKey {
    public_key: String,
}

async fn new_key(State(app): St, auth: Auth, Input(k): Input<NewKey>) -> Res<impl IntoResponse> {
    auth.scope("upload")?;
    let key = wprs_format::public_key(&k.public_key).map_err(|e| Problem::bad(e.to_string()))?;
    let fingerprint = wprs_format::fingerprint(&key).map_err(|e| Problem::bad(e.to_string()))?;
    let fp = fingerprint.clone();
    let sql = "INSERT INTO author_keys (fingerprint, user_id, public_key, created_at) \
               VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING";
    let added = app.db.run(move |c| Ok(c.execute(sql, params![fp, auth.user, key, now()])?)).await?;
    require(added > 0, "this key is already registered")?;
    Ok((StatusCode::CREATED, Json(json!({"fingerprint": fingerprint}))))
}

async fn remove(State(app): St, auth: Auth, Path((what, id)): Path<(String, String)>) -> Res<StatusCode> {
    let sql = match what.as_str() {
        "tokens" => auth
            .browser()
            .map(|()| "DELETE FROM tokens WHERE id = ?1 AND user_id = ?2 AND device_id IS NULL")?,
        "devices" => auth.scope("devices").map(|()| "DELETE FROM devices WHERE id = ?1 AND user_id = ?2")?,
        "author-keys" => auth
            .scope("upload")
            .map(|()| "DELETE FROM author_keys WHERE fingerprint = ?1 AND user_id = ?2")?,
        _ => return Err(Problem::missing("collection")),
    };
    let (user, gone) = (auth.user, vec![id.clone()]);
    let epoch = app.db.run(move |c| {
        changed(c.execute(sql, params![id, user])?, "item")?;
        Ok(if what == "devices" { revoked(c, user)? } else { None })
    });
    if let Some(epoch) = epoch.await? {
        app.hub.kick(&gone);
        app.hub.send(user, Note { event: "epoch", device: None, value: epoch });
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn revoked(c: &Connection, user: i64) -> Res<Option<i64>> {
    let sql = "UPDATE sync_heads SET rotate = 1 WHERE user_id = ?1 RETURNING epoch";
    Ok(c.query_row(sql, [user], |r| r.get(0)).optional()?)
}
