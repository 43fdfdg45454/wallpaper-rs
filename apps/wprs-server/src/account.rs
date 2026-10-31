use crate::config::Registration;
use crate::creds::{self, check_handle, check_password, digest};
use crate::db::now;
use crate::error::{Input, Problem, Res, require};
use crate::net::{Client, FAILS, LOGIN};
use crate::{App, Router, St};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Deserialize;
use serde_json::json;

mod password;
mod session;

use password::{forget, password};
pub use session::profile;
use session::{login, logout, me};

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me).delete(forget))
        .route("/api/v1/me/password", post(password))
}

#[derive(Deserialize)]
pub struct Register {
    pub handle: String,
    pub password: String,
    #[serde(default)]
    pub invite: String,
}

pub fn insert_user(c: &Connection, handle: &str, hash: Option<&str>, role: &str, quota: u64) -> Res<i64> {
    let sql = "INSERT INTO users (handle, password_hash, role, quota_bytes, created_at) \
               VALUES (?1, ?2, ?3, ?4, ?5)";
    match c.execute(sql, params![handle, hash, role, quota as i64, now()]) {
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::ConstraintViolation => {
            Err(Problem::conflict("handle_taken", "this handle is taken"))
        }
        done => Ok(done.map(|_| c.last_insert_rowid())?),
    }
}

pub async fn create_user(app: &App, client: &Client, r: Register) -> Res<i64> {
    app.limits.take(&LOGIN, &client.key())?;
    check_handle(&r.handle)?;
    check_password(&r.handle, &r.password)?;
    let invite = r.invite.trim().to_string();
    let closed = "registration is closed: ask an administrator for an account";
    match app.cfg.registration {
        Registration::Closed => return Err(Problem::forbidden(closed)),
        Registration::Invite => require(!invite.is_empty(), "invite: this server needs an invitation code")?,
        Registration::Open => {}
    }
    let (hash, quota) = (creds::hash(app, r.password).await?, app.cfg.user_quota);
    let sql = "UPDATE invites SET uses_left = uses_left - 1 WHERE code_hash = ?1 AND uses_left > 0 \
               AND expires_at > ?2 RETURNING role";
    app.db
        .run(move |c| {
            let tx = c.transaction()?;
            let role = match invite.is_empty() {
                true => Some("user".to_string()),
                false => tx.query_row(sql, params![digest(&invite), now()], |r| r.get(0)).optional()?,
            };
            let role =
                role.ok_or_else(|| Problem::forbidden("the invitation is unknown, used up or expired"))?;
            let id = insert_user(&tx, &r.handle, Some(&hash), &role, quota)?;
            tx.commit()?;
            Ok(id)
        })
        .await
}

pub async fn authenticate(app: &App, client: &Client, handle: &str, password: String) -> Res<i64> {
    app.limits.take(&LOGIN, &client.key())?;
    let account = handle.trim().to_lowercase();
    app.limits.allow(&FAILS, &account)?;
    let (key, sql) =
        (account.clone(), "SELECT id, password_hash, status = 'active' FROM users WHERE handle = ?1");
    let row = app
        .db
        .run(move |c| Ok(c.query_row(sql, [key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?));
    let row: Option<(i64, Option<String>, bool)> = row.await?;
    match (creds::verify(app, row.as_ref().and_then(|r| r.1.clone()), password).await?, row) {
        (true, Some((id, _, true))) => Ok(id),
        (true, Some(_)) => Err(Problem::forbidden("this account is suspended")),
        _ => {
            let _ = app.limits.take(&FAILS, &account);
            Err(Problem::new(StatusCode::UNAUTHORIZED, "bad_credentials", "wrong handle or password"))
        }
    }
}

async fn register(State(app): St, client: Client, Input(r): Input<Register>) -> Res<impl IntoResponse> {
    let handle = r.handle.clone();
    let id = create_user(&app, &client, r).await?;
    Ok((StatusCode::CREATED, Json(json!({"id": id, "handle": handle}))))
}
