use super::{fail, receive, tmp, waiting};
use crate::auth::{Auth, Role};
use crate::config::Publish;
use crate::creds::{hex, random};
use crate::db::rows;
use crate::error::{Problem, Res, require};
use crate::review::{approve, record};
use crate::validate::validate;
use crate::{App, Reply, St};
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use rusqlite::params;
use serde_json::Value;
use std::fs;
use std::sync::Arc;

pub(super) async fn put(
    State(app): St,
    auth: Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Reply {
    auth.scope("upload")?;
    let (size, sha) =
        waiting(&app, auth.user, &id, "open").await?.ok_or_else(|| Problem::missing("open upload"))?;
    let length = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse::<i64>().ok());
    if length != Some(size) {
        return Err(Problem::new(
            StatusCode::LENGTH_REQUIRED,
            "length",
            "Content-Length must equal the declared size",
        ));
    }
    let _turn = app.pieces.enter(auth.user, &id)?;
    let staging = tmp(&app, &format!("{id}.part-{}", hex(&random::<8>())));
    let received = receive(body.into_data_stream(), &staging, size as u64).await;
    if !matches!(received, Ok((hash, total)) if hash == sha && total == size as u64) {
        let _ = fs::remove_file(&staging);
        let detail = "the body does not match the declared size and SHA-256";
        fail(&app, &id, detail).await;
        return Err(Problem::bad(detail));
    }
    fs::rename(&staging, tmp(&app, &id))?;
    let (key, sql) = (id.clone(), "UPDATE uploads SET state = 'received' WHERE id = ?1 AND state = 'open'");
    app.db.run(move |c| Ok(c.execute(sql, [key])?)).await?;
    status(&app, auth.user, id).await
}

pub(super) async fn finalize(State(app): St, auth: Auth, Path(id): Path<String>) -> Reply {
    auth.scope("upload")?;
    finish(&app, &auth, id).await
}

pub(super) async fn finish(app: &Arc<App>, auth: &Auth, id: String) -> Reply {
    let busy = || Problem::conflict("upload_state", "the upload is not complete or was already finalized");
    let (size, sha) = waiting(app, auth.user, &id, "received").await?.ok_or_else(busy)?;
    let thumb = tmp(app, &format!("{id}.jpg"));
    let checked = match validate(app, auth.user, &tmp(app, &id), &thumb).await {
        Ok(report) => record(app, auth.user, &id, &sha, size, report).await,
        Err(problem) => Err(problem),
    };
    let _ = fs::remove_file(&thumb);
    let policy = app.cfg.auto_publish;
    let trusted = policy == Publish::All || (policy == Publish::Trusted && auth.role >= Role::Trusted);
    match checked {
        Err(problem) => fail(app, &id, &problem.detail).await,
        Ok(case) if trusted && app.repo.ready() => {
            if let Err(problem) = approve(app, case).await {
                tracing::warn!(detail = %problem.detail, "automatic publication failed");
            }
        }
        Ok(_) => {}
    }
    status(app, auth.user, id).await
}

pub async fn status(app: &App, user: i64, id: String) -> Reply {
    let sql = "SELECT u.id, CASE v.state WHEN 'pending' THEN 'pending_review' ELSE COALESCE(v.state, \
               u.state) END AS state, u.errors, v.package_id AS package, v.version FROM uploads u \
               LEFT JOIN versions v ON v.id = u.version_id WHERE u.id = ?1 AND u.user_id = ?2";
    let found = app.db.run(move |c| rows(c, sql, params![id, user])).await?.pop();
    let mut found = found.ok_or_else(|| Problem::missing("upload"))?;
    let errors: Vec<Value> = found["errors"].as_str().map(|e| vec![e.into()]).unwrap_or_default();
    found["errors"] = errors.into();
    Ok(Json(found))
}

pub(super) async fn show(State(app): St, auth: Auth, Path(id): Path<String>) -> Reply {
    auth.scope("upload")?;
    status(&app, auth.user, id).await
}

pub(super) async fn cancel(State(app): St, auth: Auth, Path(id): Path<String>) -> Res<StatusCode> {
    auth.scope("upload")?;
    let (user, key, sql) =
        (auth.user, id.clone(), "DELETE FROM uploads WHERE id = ?1 AND user_id = ?2 AND state != 'done'");
    let gone = app.db.run(move |c| Ok(c.execute(sql, params![key, user])?)).await?;
    require(gone > 0, "only unfinished uploads can be cancelled")?;
    let _ = fs::remove_file(tmp(&app, &id));
    Ok(StatusCode::NO_CONTENT)
}
