use super::{allowed, fail, finish, receive, tmp};
use crate::auth::Auth;
use crate::creds::{hex, random};
use crate::db::{DAY, now};
use crate::error::{Problem, Res, require};
use crate::{App, Reply, St};
use axum::extract::{Multipart, State};
use rusqlite::params;
use std::fs;
use std::time::Duration;

async fn reserve(app: &App, user: i64) -> Res<(String, u64)> {
    let (per_day, max) = (app.cfg.uploads_per_day as i64, app.cfg.package_max as i64);
    app.db
        .run(move |c| {
            let room = allowed(c, user, per_day)?.min(max);
            let (id, t) = (hex(&random::<16>()), now());
            let sql = "INSERT INTO uploads (id, user_id, size, sha256, state, created_at, expires_at) \
                       VALUES (?1, ?2, ?3, '', 'open', ?4, ?5)";
            c.execute(sql, params![id, user, room, t, t + DAY])?;
            Ok((id, room as u64))
        })
        .await
}

pub(super) async fn package(app: &App, form: &mut Multipart, id: &str, room: u64) -> Res<()> {
    let late = |_| Problem::bad("the form took longer than 30 seconds to arrive");
    let wait = Duration::from_secs(30);
    for _ in 0..8 {
        let next = tokio::time::timeout(wait, form.next_field()).await.map_err(late)?;
        let Some(mut field) = next.map_err(|e| Problem::bad(e.body_text()))? else { break };
        if field.name() != Some("package") {
            let skip = async {
                let mut size = 0;
                while let Some(chunk) = field.chunk().await.map_err(|e| Problem::bad(e.body_text()))? {
                    size += chunk.len();
                    require(size <= 4096, "a form field is larger than 4 KiB")?;
                }
                Ok::<_, Problem>(())
            };
            tokio::time::timeout(wait, skip).await.map_err(late)??;
            continue;
        }
        let staging = tmp(app, &format!("{id}.part"));
        let (sha, size) = receive(field, &staging, room).await?;
        let (key, sql) = (
            id.to_string(),
            "UPDATE uploads SET size = ?2, sha256 = ?3, state = 'received' \
                                             WHERE id = ?1 AND state = 'open'",
        );
        let stored = app.db.run(move |c| {
            let blocked = "SELECT COUNT(*) FROM blocklist WHERE sha256 = ?1";
            if c.query_row(blocked, [&sha], |r| r.get::<_, i64>(0))? > 0 {
                return Err(Problem::forbidden("this file is on the blocklist"));
            }
            Ok(c.execute(sql, params![key, size as i64, sha])?)
        });
        require(stored.await? == 1, "the upload was cancelled")?;
        return Ok(fs::rename(&staging, tmp(app, id))?);
    }
    Err(Problem::bad("choose a .wpak file to upload"))
}

pub(super) async fn web(State(app): St, auth: Auth, mut form: Multipart) -> Reply {
    auth.scope("upload")?;
    let (id, room) = reserve(&app, auth.user).await?;
    if let Err(problem) = package(&app, &mut form, &id, room).await {
        let _ = fs::remove_file(tmp(&app, &format!("{id}.part")));
        fail(&app, &id, &problem.detail).await;
        return Err(problem);
    }
    finish(&app, &auth, id).await
}
