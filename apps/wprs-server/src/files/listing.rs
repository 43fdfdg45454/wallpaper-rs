use super::{PAGE, device, found, held, used};
use crate::auth::Auth;
use crate::db::{now, rows};
use crate::error::{Input, Problem, require};
use crate::sync::head;
use crate::{Reply, St};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub(super) struct After {
    #[serde(default)]
    after: String,
}

pub(super) async fn list(State(app): St, auth: Auth, Query(a): Query<After>) -> Reply {
    let (user, quota) = (device(&auth, "")?, app.cfg.files_quota);
    let sql = "SELECT id, size, received, created_at FROM sync_blobs WHERE user_id = ?1 AND id > ?2 \
               ORDER BY id LIMIT ?3";
    app.db
        .run(move |c| {
            let mut items = rows(c, sql, params![user, a.after, PAGE as i64 + 1])?;
            let more = items.len() > PAGE;
            items.truncate(PAGE);
            let used = used(c, user)?;
            Ok(Json(json!({"items": items, "more": more, "used_bytes": used, "quota_bytes": quota})))
        })
        .await
}

#[derive(Deserialize)]
pub(super) struct Open {
    size: i64,
}

pub(super) async fn open(State(app): St, auth: Auth, Path(id): Path<String>, Input(o): Input<Open>) -> Reply {
    let (user, quota) = (device(&auth, &id)?, app.cfg.files_quota as i64);
    require(o.size > 0, "size: at least 1 byte")?;
    let key = id.clone();
    let (size, received) = app
        .db
        .run(move |c| {
            require(head(c, user)?.encrypted, "private files need end-to-end encryption")?;
            let tx = c.transaction()?;
            let stored = match found(&tx, user, &key) {
                Ok((size, _)) if size != o.size => {
                    return Err(Problem::conflict("size", "a file with this id has another size"));
                }
                Ok(stored) => stored,
                Err(_) if used(&tx, user)? + o.size > quota => {
                    let detail = "the quota of private files is full";
                    return Err(Problem::new(StatusCode::PAYLOAD_TOO_LARGE, "quota", detail));
                }
                Err(_) => {
                    let sql =
                        "INSERT INTO sync_blobs (user_id, id, size, created_at) VALUES (?1, ?2, ?3, ?4)";
                    tx.execute(sql, params![user, key, o.size, now()])?;
                    (o.size, 0)
                }
            };
            tx.commit()?;
            Ok(stored)
        })
        .await?;
    let received = held(&app, user, &id, received).await?;
    Ok(Json(json!({"id": id, "size": size, "received": received})))
}
