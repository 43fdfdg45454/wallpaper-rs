use crate::auth::DEVICE_SCOPES;
use crate::creds::{self, digest, hex, random};
use crate::db::{MINUTE, now};
use crate::error::{Input, Problem, Res, require};
use crate::net::{Client, LOGIN};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/auth/device/start", post(start))
        .route("/api/v1/auth/device/token", post(token))
        .route("/api/v1/auth/device/approve", post(crate::approval::approve))
        .route("/api/v1/auth/device/preview", post(crate::approval::preview))
}

#[derive(Deserialize)]
pub struct NewDevice {
    pub platform: String,
    #[serde(default)]
    pub enc_key: Option<String>,
    #[serde(default)]
    pub sig_key: Option<String>,
    #[serde(default)]
    pub client: String,
}

type Keys = (Option<Vec<u8>>, Option<Vec<u8>>);
type Code = (String, Option<i64>, i64, i64, i64, String, Option<Vec<u8>>, Option<Vec<u8>>);

fn keys(d: &NewDevice) -> Res<Keys> {
    require(
        ["linux", "android", "other"].contains(&d.platform.as_str()),
        "platform: linux, android or other",
    )?;
    let decode = |key: &Option<String>| {
        let raw = key.as_deref().map(|k| BASE64_STANDARD.decode(k).ok().filter(|b| b.len() == 32));
        raw.map(|k| k.ok_or_else(|| Problem::bad("enc_key and sig_key: base64 of 32 bytes"))).transpose()
    };
    Ok((decode(&d.enc_key)?, decode(&d.sig_key)?))
}

pub fn create(c: &Connection, user: i64, d: &NewDevice) -> Res<Value> {
    let (enc, sig) = keys(d)?;
    enroll(c, user, &d.platform, enc, sig)
}

fn enroll(
    c: &Connection,
    user: i64,
    platform: &str,
    enc: Option<Vec<u8>>,
    sig: Option<Vec<u8>>,
) -> Res<Value> {
    let (id, token, t) = (hex(&random::<16>()), creds::token('d'), now());
    let number: i64 =
        c.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM devices WHERE user_id = ?1", [user], |r| {
            r.get(0)
        })?;
    let sql = "INSERT INTO devices (id, user_id, number, platform, enc_key, sig_key, created_at) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
    c.execute(sql, params![id, user, number, platform, enc, sig, t])?;
    let name = format!("Device {number}");
    let sql = "INSERT INTO tokens (user_id, device_id, name, hash, scopes, created_at) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
    c.execute(sql, params![user, id, name, digest(&token), DEVICE_SCOPES, t])?;
    Ok(json!({"token": token, "device_id": id, "device_name": name, "scopes": DEVICE_SCOPES}))
}

async fn start(State(app): St, client: Client, Input(d): Input<NewDevice>) -> Reply {
    app.limits.take(&LOGIN, &client.key())?;
    let (enc, sig) = keys(&d)?;
    let name = d.client.trim().to_string();
    require(
        name.chars().count() <= 64 && !name.chars().any(char::is_control),
        "client: at most 64 characters",
    )?;
    let (code, user_code) = (hex(&random::<32>()), creds::user_code());
    let (hash, shown) = (digest(&code), user_code.clone());
    app.db
        .run(move |c| {
            let sql = "INSERT INTO device_codes (code_hash, user_code, platform, enc_key, sig_key, client, \
                   created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
            let t = now();
            Ok(c.execute(sql, params![hash, shown, d.platform, enc, sig, name, t, t + 10 * MINUTE])?)
        })
        .await?;
    let complete = app.cfg.url(&format!("/link?code={user_code}"));
    Ok(Json(json!({
        "device_code": code, "user_code": user_code, "verification_uri": app.cfg.url("/link"),
        "verification_uri_complete": complete, "interval": 5, "expires_in": 600,
    })))
}

#[derive(Deserialize)]
struct Poll {
    device_code: String,
}

fn pending(code: &'static str, detail: &str) -> Problem {
    Problem::new(StatusCode::BAD_REQUEST, code, detail)
}

async fn token(State(app): St, Input(p): Input<Poll>) -> Reply {
    let hash = digest(p.device_code.trim());
    app.db
        .run(move |c| {
            let t = now();
            let sql = "SELECT state, user_id, interval_s, polled_at, expires_at, platform, enc_key, sig_key \
                       FROM device_codes WHERE code_hash = ?1";
            let read = |r: &Row| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
            };
            let row: Option<Code> = c.query_row(sql, [&hash], read).optional()?;
            let Some((state, user, interval, polled, expires, platform, enc, sig)) = row else {
                return Err(pending("invalid_grant", "unknown device code"));
            };
            if expires < t {
                c.execute("DELETE FROM device_codes WHERE code_hash = ?1", [&hash])?;
                return Err(pending("expired_token", "the code expired, start again"));
            }
            if t - polled < interval * 1000 {
                let sql = "UPDATE device_codes SET interval_s = interval_s + 5, polled_at = ?2 \
                           WHERE code_hash = ?1";
                c.execute(sql, params![hash, t])?;
                return Err(pending("slow_down", "poll less often: add 5 seconds to the interval"));
            }
            c.execute("UPDATE device_codes SET polled_at = ?2 WHERE code_hash = ?1", params![hash, t])?;
            match (state.as_str(), user) {
                ("approved", Some(user)) => {
                    c.execute("DELETE FROM device_codes WHERE code_hash = ?1", [&hash])?;
                    enroll(c, user, &platform, enc, sig).map(Json)
                }
                ("denied", _) => {
                    c.execute("DELETE FROM device_codes WHERE code_hash = ?1", [&hash])?;
                    Err(pending("access_denied", "the request was denied"))
                }
                _ => Err(pending("authorization_pending", "waiting for approval")),
            }
        })
        .await
}
