use crate::App;
use crate::creds::{digest, well_formed};
use crate::db::{DAY, HOUR, MINUTE, now};
use crate::error::{Problem, Res};
use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    User,
    Trusted,
    Moderator,
    Admin,
}

pub const ROLES: [&str; 4] = ["user", "trusted", "moderator", "admin"];
pub const SCOPES: [&str; 6] = ["catalog:read", "upload", "sync", "devices", "mod", "admin"];
pub const DEVICE_SCOPES: &str = "upload sync";

impl Role {
    pub fn parse(text: &str) -> Role {
        [Role::User, Role::Trusted, Role::Moderator, Role::Admin]
            [ROLES.iter().position(|r| *r == text).unwrap_or(0)]
    }

    pub fn name(self) -> &'static str {
        ROLES[self as usize]
    }
}

#[derive(Clone, Debug)]
pub struct Auth {
    pub user: i64,
    pub handle: String,
    pub role: Role,
    pub device: Option<String>,
    pub scopes: Option<String>,
    pub token: Option<i64>,
    pub session: Option<Vec<u8>>,
}

impl Auth {
    pub fn scope(&self, scope: &str) -> Res<()> {
        let ok = self.scopes.as_deref().is_none_or(|s| s.split(' ').any(|x| x == scope));
        if ok { Ok(()) } else { Err(Problem::forbidden(format!("the token lacks the {scope} scope"))) }
    }

    pub fn device(&self) -> Res<&str> {
        self.scope("sync")?;
        self.device.as_deref().ok_or_else(|| Problem::forbidden("this call needs a device token"))
    }

    pub fn at_least(&self, role: Role) -> Res<()> {
        if self.role < role {
            return Err(Problem::forbidden(format!("this needs the {} role", role.name())));
        }
        self.scope(if role == Role::Admin { "admin" } else { "mod" })
    }

    pub fn browser(&self) -> Res<()> {
        let ok = self.session.is_some();
        if ok { Ok(()) } else { Err(Problem::forbidden("do this from the account page of the web site")) }
    }
}

impl FromRequestParts<Arc<App>> for Auth {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Res<Auth> {
        let found = <Auth as OptionalFromRequestParts<Arc<App>>>::from_request_parts(parts, app).await?;
        found.ok_or_else(Problem::unauthorized)
    }
}

impl OptionalFromRequestParts<Arc<App>> for Auth {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Res<Option<Auth>> {
        if let Some(found) = parts.extensions.get::<Auth>() {
            return Ok(Some(found.clone()));
        }
        let header = |name| parts.headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
        let bearer =
            header(AUTHORIZATION).and_then(|v| v.strip_prefix("Bearer ")).map(|t| t.trim().to_string());
        let name = app.cfg.cookie();
        let cookie = header(COOKIE).into_iter().flat_map(|v| v.split(';')).find_map(|pair| {
            pair.trim().strip_prefix(name).and_then(|rest| rest.strip_prefix('=')).map(String::from)
        });
        let auth = match (bearer, cookie) {
            (Some(token), _) if well_formed(&token) => Some(
                app.db.run(move |c| by_token(c, &digest(&token))).await?.ok_or_else(Problem::unauthorized)?,
            ),
            (Some(_), _) => return Err(Problem::unauthorized()),
            (None, Some(value)) => app.db.run(move |c| by_session(c, &digest(&value))).await?,
            (None, None) => None,
        };
        if let Some(found) = &auth {
            parts.extensions.insert(found.clone());
        }
        Ok(auth)
    }
}

fn user(row: &Row, at: usize) -> rusqlite::Result<(i64, String, Role)> {
    Ok((row.get(at)?, row.get(at + 1)?, Role::parse(&row.get::<_, String>(at + 2)?)))
}

fn by_token(conn: &Connection, hash: &[u8]) -> Res<Option<Auth>> {
    let t = now();
    let sql = "SELECT t.id, t.device_id, t.scopes, t.used_at, u.id, u.handle, u.role FROM tokens t \
               JOIN users u ON u.id = t.user_id WHERE t.hash = ?1 AND (t.expires_at IS NULL \
               OR t.expires_at > ?2) AND u.status = 'active'";
    let row = conn.query_row(sql, params![hash, t], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<i64>>(3)?, user(r, 4)?))
    });
    let Some((id, device, scopes, used, (user, handle, role))) = row.optional()? else { return Ok(None) };
    if used.is_none_or(|u| t - u > HOUR) {
        conn.execute("UPDATE tokens SET used_at = ?2 WHERE id = ?1", params![id, t])?;
        conn.execute("UPDATE devices SET seen_at = ?2 WHERE id = ?1", params![device, t])?;
    }
    Ok(Some(Auth { user, handle, role, device, scopes: Some(scopes), token: Some(id), session: None }))
}

fn by_session(conn: &Connection, hash: &[u8]) -> Res<Option<Auth>> {
    let t = now();
    let sql = "SELECT s.seen_at, u.id, u.handle, u.role FROM sessions s JOIN users u ON u.id = s.user_id \
               WHERE s.id_hash = ?1 AND s.seen_at > ?2 AND s.created_at > ?3 AND u.status = 'active'";
    let row = conn
        .query_row(sql, params![hash, t - 7 * DAY, t - 30 * DAY], |r| Ok((r.get::<_, i64>(0)?, user(r, 1)?)));
    let Some((seen, (user, handle, role))) = row.optional()? else { return Ok(None) };
    if t - seen > MINUTE {
        conn.execute("UPDATE sessions SET seen_at = ?2 WHERE id_hash = ?1", params![hash, t])?;
    }
    let session = Some(hash.to_vec());
    Ok(Some(Auth { user, handle, role, device: None, scopes: None, token: None, session }))
}
