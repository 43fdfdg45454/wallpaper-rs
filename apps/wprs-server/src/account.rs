use crate::auth::Auth;
use crate::config::Registration;
use crate::creds::{self, check_handle, check_password, digest};
use crate::db::{MINUTE, changed, now, rows};
use crate::device::{NewDevice, create};
use crate::error::{Input, Problem, Res, require};
use crate::mailbox::Note;
use crate::me::revoked;
use crate::net::{Client, FAILS, LOGIN};
use crate::{App, Reply, Router, St};
use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Deserialize;
use serde_json::json;

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

#[derive(Deserialize)]
struct Login {
    handle: String,
    password: String,
    device: NewDevice,
}

async fn login(State(app): St, client: Client, Json(l): Json<Login>) -> Reply {
    let user = authenticate(&app, &client, &l.handle, l.password).await?;
    Ok(Json(app.db.run(move |c| create(c, user, &l.device)).await?))
}

async fn logout(State(app): St, auth: Auth) -> Res<impl IntoResponse> {
    let (token, session) = (auth.token, auth.session);
    let gone = app.db.run(move |c| {
        c.execute("DELETE FROM tokens WHERE id = ?1", [token])?;
        Ok(c.execute("DELETE FROM sessions WHERE id_hash = ?1", [session])?)
    });
    gone.await?;
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, creds::cookie(&app.cfg, "", 0))]))
}

pub fn profile(c: &Connection, user: i64) -> Res<serde_json::Value> {
    let sql = "SELECT id, handle, role, created_at, quota_bytes, (SELECT COALESCE(SUM(size), 0) \
               FROM versions WHERE uploaded_by = ?1 AND state != 'rejected') AS used_bytes FROM users \
               WHERE id = ?1";
    rows(c, sql, [user])?.pop().ok_or_else(|| Problem::missing("account"))
}

async fn me(State(app): St, auth: Auth) -> Reply {
    Ok(Json(app.db.run(move |c| profile(c, auth.user)).await?))
}

async fn confirm(app: &App, auth: &Auth, client: &Client, password: String) -> Res<()> {
    app.limits.take(&LOGIN, &client.key())?;
    let account = auth.handle.to_lowercase();
    app.limits.allow(&FAILS, &account)?;
    let (user, session) = (auth.user, auth.session.clone());
    let sql = "SELECT u.password_hash, s.created_at FROM users u LEFT JOIN sessions s ON s.id_hash = ?2 \
               WHERE u.id = ?1";
    let read = move |c: &mut Connection| {
        Ok(c.query_row(sql, params![user, session], |r| Ok((r.get(0)?, r.get(1)?)))?)
    };
    let (stored, since): (Option<String>, Option<i64>) = app.db.run(read).await?;
    let fresh = since.is_some_and(|at| now() - at < 10 * MINUTE);
    let detail = match stored.is_none() {
        true => "an account without a password needs a sign-in of the last 10 minutes: sign in again",
        false => "the password is wrong",
    };
    let ok = match stored {
        None => fresh,
        stored => creds::verify(app, stored, password).await?,
    };
    if !ok {
        let _ = app.limits.take(&FAILS, &account);
        return Err(Problem::forbidden(detail));
    }
    Ok(())
}

#[derive(Deserialize)]
struct Forget {
    #[serde(default)]
    password: String,
}

async fn forget(
    State(app): St,
    auth: Auth,
    client: Client,
    Input(f): Input<Forget>,
) -> Res<impl IntoResponse> {
    auth.browser()?;
    confirm(&app, &auth, &client, f.password).await?;
    let files = crate::files::dir(&app, auth.user);
    app.db
        .run(move |c| changed(c.execute("DELETE FROM users WHERE id = ?1", [auth.user])?, "account"))
        .await?;
    let _ = tokio::fs::remove_dir_all(files).await;
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, creds::cookie(&app.cfg, "", 0))]))
}

#[derive(Deserialize)]
struct Change {
    current: String,
    new: String,
    #[serde(default)]
    devices: bool,
}

async fn password(
    State(app): St,
    auth: Auth,
    client: Client,
    Input(change): Input<Change>,
) -> Res<impl IntoResponse> {
    auth.browser()?;
    confirm(&app, &auth, &client, change.current).await?;
    check_password(&auth.handle, &change.new)?;
    let (hash, user, devices) = (creds::hash(&app, change.new).await?, auth.user, change.devices);
    let (session, gone, epoch) = app
        .db
        .run(move |c| {
            let tx = c.transaction()?;
            tx.execute("UPDATE users SET password_hash = ?2 WHERE id = ?1", params![user, hash])?;
            tx.execute("DELETE FROM sessions WHERE user_id = ?1", [user])?;
            tx.execute("DELETE FROM tokens WHERE user_id = ?1 AND device_id IS NULL", [user])?;
            tx.execute("DELETE FROM device_codes WHERE user_id = ?1", [user])?;
            let sql = "DELETE FROM devices WHERE user_id = ?1 AND ?2 RETURNING id";
            let gone = tx
                .prepare(sql)?
                .query_map(params![user, devices], |r| r.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            let epoch = if gone.is_empty() { None } else { revoked(&tx, user)? };
            let session = creds::session(&tx, user)?;
            tx.commit()?;
            Ok((session, gone, epoch))
        })
        .await?;
    app.hub.kick(&gone);
    if let Some(epoch) = epoch {
        app.hub.send(user, Note { event: "epoch", device: None, value: epoch });
    }
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, creds::cookie(&app.cfg, &session, 30 * 86400))]))
}
