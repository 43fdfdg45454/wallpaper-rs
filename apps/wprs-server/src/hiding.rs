use crate::admin::choose;
use crate::auth::{Auth, Role};
use crate::db::{audit, changed, now};
use crate::error::Res;
use crate::error::{Input, Problem, require};
use crate::moderation::open_cases;
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

pub fn routes() -> Router {
    Router::new()
        .route("/mod", get(moderation))
        .route(
            "/api/v1/mod/hidden",
            get(hidden).post(|s: St, a: Auth, Input(h): Input<Hide>| hide(s, a, h, true)),
        )
        .route(
            "/api/v1/mod/hidden/{id}",
            delete(|s: St, a: Auth, Path(id)| hide(s, a, Hide { id, reason: String::new() }, false)),
        )
}

#[derive(Deserialize)]
pub struct Hide {
    id: String,
    #[serde(default)]
    reason: String,
}

async fn hidden(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Moderator)?;
    let sql = "SELECT id, hidden AS reason, hidden_at FROM packages WHERE hidden IS NOT NULL ORDER BY hidden_at DESC";
    app.db.items(sql, vec![]).await
}

async fn hide(State(app): St, auth: Auth, h: Hide, on: bool) -> Reply {
    auth.at_least(Role::Moderator)?;
    let (id, reason) = (h.id, h.reason.trim().to_string());
    require(!on || reason.chars().count() >= 10, "reason: at least 10 characters for the author")?;
    let (key, shared) = (id.clone(), app.clone());
    let sql = "UPDATE packages SET hidden = ?2, hidden_at = ?3 WHERE id = ?1 AND (hidden IS NULL) = ?4";
    app.db
        .run(move |c| {
            let (text, at) = (on.then_some(reason.clone()), on.then(now));
            let what = if on { "visible package" } else { "hidden package" };
            changed(c.execute(sql, params![key, text, at, on])?, what)?;
            audit(c, Some(auth.user), if on { "hide" } else { "unhide" }, &key, &reason)?;
            Ok(c.execute("UPDATE repo_state SET dirty = 1", [])?)
        })
        .await?;
    let key = id.clone();
    let marked =
        tokio::task::spawn_blocking(move || shared.repo.hide(&key, on)).await.map_err(Problem::internal)?;
    if let Err(e) = marked {
        tracing::info!("a hidden package is not in the repository yet: {e}");
    }
    app.dirty.notify_one();
    Ok(Json(json!({"id": id, "hidden": on})))
}

async fn moderation(State(app): St, auth: Option<Auth>) -> Res<Response> {
    let Some(auth) = auth.filter(|a| a.role >= Role::Moderator) else {
        return Ok(Redirect::to("/login?next=/mod").into_response());
    };
    let sql = "SELECT id, hidden AS reason FROM packages WHERE hidden IS NOT NULL ORDER BY hidden_at DESC";
    let (cases, hidden) = app.db.run(move |c| Ok((open_cases(c)?, crate::db::rows(c, sql, [])?))).await?;
    let decisions = ["approve", "reject", "remove", "revoke", "restore", "dismiss"];
    let reasons = ["other", "malware", "license", "takedown", "broken"];
    let body = html! {
        @if cases.is_empty() { p { "No open cases." } }
        @for case in &cases { section.card {
            h2 { "Case " (case["id"]) ": " (text(case, "reason")) " · " (text(case, "title")) }
            img width="320" src={ "/media/" (text(case, "sha256")) ".jpg" }
                alt={ "Preview of " (text(case, "title")) };
            dl {
                dt { "Package" } dd { code { (text(case, "package")) " " (text(case, "version")) } }
                dt { "State" } dd { (text(case, "state")) }
                dt { "Uploaded by" } dd { (text(case, "uploader")) }
                dt { "Details" } dd { (text(case, "detail")) }
            }
            form.stack method="post" action={ "/api/v1/mod/cases/" (case["id"]) "/decision?back=/mod" } {
                label { "Decision" (choose("decision", "Decision", &decisions, "approve")) }
                label { "Reason sent to the author (required to reject, remove or revoke)"
                    textarea name="statement" rows="3" {} }
                label { "Revocation reason" (choose("reason", "Revocation reason", &reasons, "other")) }
                label { input type="checkbox" name="strike" value="true"; "Give the uploader a strike" }
                button { "Decide" }
            }
        } }
        h2 { "Hidden packages" }
        p.muted { "Hidden packages leave the catalog and search; whoever installed them keeps them and their updates." }
        table { tbody { @for h in &hidden { tr {
            td { a href={ "/packages/" (text(h, "id")) } { code { (text(h, "id")) } } } td { (text(h, "reason")) }
            td { form method="post" action={ "/api/v1/mod/hidden/" (text(h, "id")) "?_method=DELETE&back=/mod" } {
                button { "Show again" }
            } }
        } } } }
        form method="post" action="/api/v1/mod/hidden?back=/mod" {
            label { "Package id" input name="id" required; }
            label { "Reason sent to the author" input name="reason" required minlength="10"; }
            button { "Hide" }
        }
        h2 { "Blocklist" }
        form method="post" action="/api/v1/mod/blocklist?back=/mod" {
            label { "SHA-256 of a .wpak" input name="sha256" required pattern="[0-9a-f]{64}"; }
            label { "Reason" input name="reason" required; }
            button { "Block" }
        }
    };
    Ok(page(&app, Some(&auth), "Moderation queue", body).into_response())
}
