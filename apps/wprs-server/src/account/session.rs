use super::authenticate;
use crate::auth::Auth;
use crate::creds;
use crate::db::{MINUTE, now, rows};
use crate::device::{NewDevice, create};
use crate::error::{Problem, Res};
use crate::net::{Client, FAILS, LOGIN};
use crate::{App, Reply, St};
use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use rusqlite::{Connection, params};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Login {
    handle: String,
    password: String,
    device: NewDevice,
}

pub(super) async fn login(State(app): St, client: Client, Json(l): Json<Login>) -> Reply {
    let user = authenticate(&app, &client, &l.handle, l.password).await?;
    Ok(Json(app.db.run(move |c| create(c, user, &l.device)).await?))
}

pub(super) async fn logout(State(app): St, auth: Auth) -> Res<impl IntoResponse> {
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

pub(super) async fn me(State(app): St, auth: Auth) -> Reply {
    Ok(Json(app.db.run(move |c| profile(c, auth.user)).await?))
}

pub(super) async fn confirm(app: &App, auth: &Auth, client: &Client, password: String) -> Res<()> {
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
