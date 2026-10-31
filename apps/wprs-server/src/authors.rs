use crate::auth::Auth;
use crate::db::{now, one};
use crate::error::{Input, Problem, Res, require};
use crate::{App, St};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

async fn dirty(app: &Arc<App>) -> Res<()> {
    app.db.run(|c| Ok(c.execute("UPDATE repo_state SET dirty = 1", [])?)).await?;
    app.dirty.notify_one();
    Ok(())
}

pub async fn publish(app: &Arc<App>, user: i64, add: Option<String>, gone: Option<String>) -> Res<()> {
    if !app.repo.online() {
        return Ok(());
    }
    let shared = app.clone();
    let changed = tokio::task::spawn_blocking(move || {
        add.map(|key| shared.repo.register(user, &key)).transpose()?;
        gone.map(|fp| shared.repo.drop_key(user, &fp)).transpose()
    });
    changed.await.map_err(Problem::internal)??;
    dirty(app).await
}

#[derive(Deserialize)]
pub struct Rotate {
    from: String,
    public_key: String,
    signature: String,
}

pub async fn rotate(State(app): St, auth: Auth, Input(r): Input<Rotate>) -> Res<impl IntoResponse> {
    auth.scope("upload")?;
    require(app.repo.online(), "this repository binds its authors in the root: ask its operator")?;
    let key = wprs_format::public_key(&r.public_key).map_err(|e| Problem::bad(e.to_string()))?;
    let fp = wprs_format::fingerprint(&key).map_err(|e| Problem::bad(e.to_string()))?;
    let (user, from) = (auth.user, r.from.clone());
    let sql = "SELECT 1 FROM author_keys WHERE fingerprint = ?1 AND user_id = ?2";
    let owned = app.db.run(move |c| one::<i64>(c, sql, params![from, user])).await?;
    owned.ok_or_else(|| Problem::missing("author key"))?;
    let (shared, public) = (app.clone(), key.clone());
    let rotated =
        tokio::task::spawn_blocking(move || shared.repo.rotate(user, &r.from, &public, &r.signature));
    rotated.await.map_err(Problem::internal)??;
    let (sql, at, id) = (
        "INSERT INTO author_keys (fingerprint, user_id, public_key, created_at) \
                          VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
        now(),
        fp.clone(),
    );
    app.db.run(move |c| Ok(c.execute(sql, params![id, user, key, at])?)).await?;
    dirty(&app).await?;
    Ok((StatusCode::CREATED, Json(json!({"fingerprint": fp}))))
}
