use crate::auth::{Auth, Role};
use crate::db::{audit, changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::web::{page, text};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get};
use maud::html;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

const LIST: &str = "SELECT f.package_id AS id, f.note, f.at FROM featured f ORDER BY f.at DESC";

pub fn routes() -> Router {
    Router::new()
        .route("/admin/featured", get(screen))
        .route(
            "/api/v1/admin/featured",
            get(list).post(|s: St, a: Auth, Input(f): Input<Feature>| set(s, a, f, true)),
        )
        .route(
            "/api/v1/admin/featured/{id}",
            delete(|s: St, a: Auth, Path(id)| set(s, a, Feature { id, note: String::new() }, false)),
        )
}

#[derive(Deserialize)]
pub struct Feature {
    id: String,
    #[serde(default)]
    note: String,
}

async fn list(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    Ok(Json(json!({"items": app.db.run(|c| rows(c, LIST, [])).await?})))
}

async fn set(State(app): St, auth: Auth, f: Feature, on: bool) -> Reply {
    auth.at_least(Role::Admin)?;
    let (id, note) = (f.id, f.note.trim().to_string());
    let plain = note.chars().count() <= 200 && !note.contains(['<', '>', '\n', '\r']);
    require(plain, "note: at most 200 characters on one line, without < or >")?;
    let (key, words, user) = (id.clone(), note.clone(), auth.user);
    app.db
        .run(move |c| {
            if on {
                let sql = "SELECT COUNT(*) FROM versions WHERE package_id = ?1 AND state = 'published'";
                require(c.query_row(sql, [&key], |r| r.get::<_, i64>(0))? > 0, "id: a published package")?;
                let count = c.query_row("SELECT COUNT(*) FROM featured", [], |r| r.get::<_, i64>(0))?;
                let known =
                    c.query_row("SELECT COUNT(*) FROM featured WHERE package_id = ?1", [&key], |r| {
                        r.get::<_, i64>(0)
                    })?;
                require(count < 64 || known > 0, "at most 64 featured packages")?;
                let sql = "INSERT INTO featured (package_id, note, at) VALUES (?1, ?2, ?3) \
                           ON CONFLICT (package_id) DO UPDATE SET note = ?2";
                c.execute(sql, params![key, (!words.is_empty()).then_some(&words), now()])?;
            } else {
                changed(
                    c.execute("DELETE FROM featured WHERE package_id = ?1", [&key])?,
                    "featured package",
                )?;
            }
            audit(c, Some(user), if on { "feature" } else { "unfeature" }, &key, &words)?;
            Ok(c.execute("UPDATE repo_state SET dirty = 1", [])?)
        })
        .await?;
    let (shared, key) = (app.clone(), id.clone());
    let note = (!note.is_empty()).then_some(note);
    let marked = tokio::task::spawn_blocking(move || shared.repo.feature(&key, note.as_deref(), on))
        .await
        .map_err(Problem::internal)?;
    if let Err(e) = marked {
        tracing::info!("a featured package is not in the repository yet: {e}");
    }
    app.dirty.notify_one();
    Ok(Json(json!({"id": id, "featured": on})))
}

async fn screen(State(app): St, auth: Option<Auth>) -> Res<Response> {
    let Some(auth) = auth.filter(|a| a.role >= Role::Admin) else {
        return Ok(Redirect::to("/login?next=/admin/featured").into_response());
    };
    let items = app.db.run(|c| rows(c, LIST, [])).await?;
    let body = html! {
        p.muted { "Featured packages are signed into the repository; clients show them apart in their catalog." }
        table { tbody { @for i in &items { tr {
            td { a href={ "/packages/" (text(i, "id")) } { code { (text(i, "id")) } } } td { (text(i, "note")) }
            td { form method="post" action={ "/api/v1/admin/featured/" (text(i, "id")) "?_method=DELETE&back=/admin/featured" } {
                button { "Stop featuring" }
            } }
        } } } }
        form method="post" action="/api/v1/admin/featured?back=/admin/featured" {
            label { "Package id" input name="id" required; }
            label { "Note" input name="note" maxlength="200"; }
            button { "Feature" }
        }
    };
    Ok(page(&app, Some(&auth), "Featured packages", body).into_response())
}
