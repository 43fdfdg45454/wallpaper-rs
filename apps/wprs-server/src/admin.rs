use crate::auth::{Auth, ROLES, Role};
use crate::creds::{digest, hex, random};
use crate::db::{DAY, audit, changed, now, rows};
use crate::error::{Input, Problem, Res, require};
use crate::jobs::republish;
use crate::web::{page, text};
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, patch, post};
use maud::{Markup, html};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/admin/invites", get(invites).post(invite))
        .route("/api/v1/admin/invites/{id}", delete(uninvite))
        .route("/api/v1/admin/users", get(users))
        .route("/api/v1/admin/users/{id}", patch(edit))
        .route("/api/v1/admin/repo", get(repo))
        .route("/api/v1/admin/repo/publish", post(publish))
        .route("/api/v1/admin/transparency", get(transparency))
        .route("/admin", get(administration))
}

const INVITES: &str =
    "SELECT id, role, uses_left, expires_at FROM invites WHERE expires_at > ?1 AND uses_left > 0";
const USERS: &str = "SELECT id, handle, role, status, quota_bytes, created_at FROM users WHERE ?1 IN (0, id)";

async fn invites(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    app.db.items(INVITES, vec![now().into()]).await
}

#[derive(Deserialize)]
struct Invite {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    uses: Option<i64>,
    #[serde(default)]
    days: Option<i64>,
}

async fn invite(State(app): St, auth: Auth, Input(i): Input<Invite>) -> Res<impl IntoResponse> {
    auth.at_least(Role::Admin)?;
    let (role, uses, days) =
        (i.role.unwrap_or_else(|| "user".into()), i.uses.unwrap_or(1), i.days.unwrap_or(7));
    let valid = ROLES.contains(&role.as_str()) && (1..=1000).contains(&uses) && (1..=90).contains(&days);
    require(valid, "role, 1 to 1000 uses and 1 to 90 days")?;
    let (code, expires) = (hex(&random::<16>()), now() + days * DAY);
    let (hash, kept) = (digest(&code), role.clone());
    let sql = "INSERT INTO invites (code_hash, role, uses_left, expires_at, created_by) \
               VALUES (?1, ?2, ?3, ?4, ?5)";
    let id = app.db.run(move |c| {
        c.execute(sql, params![hash, kept, uses, expires, auth.user])?;
        audit(c, Some(auth.user), "invite", &kept, "")?;
        Ok(c.last_insert_rowid())
    });
    let (id, url) = (id.await?, app.cfg.url(&format!("/register?invite={code}")));
    let body = json!({"id": id, "code": code, "url": url, "role": role, "uses": uses, "expires_at": expires});
    Ok((StatusCode::CREATED, Json(body)))
}

async fn uninvite(State(app): St, auth: Auth, Path(id): Path<i64>) -> Res<StatusCode> {
    auth.at_least(Role::Admin)?;
    app.db.run(move |c| changed(c.execute("DELETE FROM invites WHERE id = ?1", [id])?, "invite")).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn users(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    app.db.items(USERS, vec![0.into()]).await
}

#[derive(Deserialize)]
struct Edit {
    role: Option<String>,
    status: Option<String>,
    quota_mib: Option<i64>,
}

async fn edit(State(app): St, auth: Auth, Path(id): Path<i64>, Input(e): Input<Edit>) -> Reply {
    auth.at_least(Role::Admin)?;
    let own = id == auth.user && (e.role.is_some() || e.status.is_some());
    require(!own, "change your own role or status from another administrator account")?;
    require(e.role.as_deref().is_none_or(|r| ROLES.contains(&r)), "role: user, trusted, moderator or admin")?;
    require(
        e.status.as_deref().is_none_or(|s| ["active", "suspended"].contains(&s)),
        "status: active or suspended",
    )?;
    require(e.quota_mib.is_none_or(|q| (0..=1 << 30).contains(&q)), "quota_mib: 0 or more")?;
    let sql = "UPDATE users SET role = COALESCE(?2, role), status = COALESCE(?3, status), quota_bytes = \
               COALESCE(?4 * 1048576, quota_bytes) WHERE id = ?1";
    app.db
        .run(move |c| {
            changed(c.execute(sql, params![id, e.role, e.status, e.quota_mib])?, "user")?;
            let detail = format!("role={:?} status={:?} quota_mib={:?}", e.role, e.status, e.quota_mib);
            audit(c, Some(auth.user), "edit-user", &id.to_string(), &detail)?;
            c.execute(
                "DELETE FROM sessions WHERE user_id = ?1 AND ?2",
                params![id, e.status.as_deref() == Some("suspended")],
            )?;
            Ok(Json(rows(c, USERS, [id])?.pop().unwrap_or_default()))
        })
        .await
}

async fn repo(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    Ok(Json(app.repo.info(&app.cfg).unwrap_or(Value::Null)))
}

async fn publish(State(app): St, auth: Auth) -> Reply {
    auth.at_least(Role::Admin)?;
    let missing = || Problem::conflict("no_repository", "create the repository first: wprs-server repo init");
    let done = republish(&app, true).await?.ok_or_else(missing)?;
    let expires = done.expires.to_rfc3339();
    Ok(Json(json!({"entry_version": done.version, "expires": expires, "packages": done.packages})))
}

async fn transparency(State(app): St, auth: Auth) -> Res<Response> {
    auth.at_least(Role::Admin)?;
    let sql = "SELECT 'notice' AS what, kind AS name, COUNT(*) AS count FROM notices GROUP BY kind \
               UNION ALL SELECT 'case', reason, COUNT(*) FROM cases GROUP BY reason \
               UNION ALL SELECT 'decision', decision, COUNT(*) FROM cases WHERE decision IS NOT NULL \
               GROUP BY decision";
    let counts = app.db.run(move |c| rows(c, sql, [])).await?;
    let lines = counts.iter().map(|r| format!("{},{},{}\n", text(r, "what"), text(r, "name"), r["count"]));
    let csv: String = std::iter::once("category,name,count\n".to_string()).chain(lines).collect();
    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8")], csv).into_response())
}

pub fn choose(name: &str, label: &str, options: &[&str], chosen: &str) -> Markup {
    html! {
        select name=(name) aria-label=(label) {
            @for o in options { option value=(o) selected[*o == chosen] { (o) } }
        }
    }
}

async fn administration(State(app): St, auth: Option<Auth>) -> Res<Response> {
    let Some(auth) = auth.filter(|a| a.role >= Role::Admin) else {
        return Ok(Redirect::to("/login?next=/admin").into_response());
    };
    let (invites, users) = app.db.run(|c| Ok((rows(c, INVITES, [now()])?, rows(c, USERS, [0])?))).await?;
    let repo = app.repo.info(&app.cfg);
    let shown = |v: &Value| v.as_str().map_or(v.to_string(), String::from);
    let body = html! {
        section.card { h2 { "Repository" }
            @if let Some(r) = &repo {
                dl { @for (k, v) in r.as_object().into_iter().flatten() { dt { (k) } dd { (shown(v)) } } }
                form method="post" action="/api/v1/admin/repo/publish?back=/admin" {
                    button { "Sign and publish now" }
                }
            } @else {
                p { "No repository yet. Create one with " code { "wprs-server repo init --name \"…\"" }
                    " and keep the printed root key offline." }
            }
        }
        section.card { h2 { "Invitations" }
            table { tbody { @for i in &invites { tr {
                td { (text(i, "role")) } td { (i["uses_left"]) " uses left" }
                td { form method="post"
                    action={ "/api/v1/admin/invites/" (i["id"]) "?_method=DELETE&back=/admin" } {
                    button { "Revoke" }
                } }
            } } } }
            form method="post" action="/api/v1/admin/invites?back=/admin" {
                label { "Role" (choose("role", "Role", &ROLES, "user")) }
                label { "Uses" input type="number" name="uses" min="1" max="1000" value="1"; }
                label { "Days" input type="number" name="days" min="1" max="90" value="7"; }
                button { "Create invitation" }
            }
        }
        section.card { h2 { "Accounts" }
            table {
                thead { tr { th { "Handle" } th { "Role and status" } } }
                tbody { @for u in &users { tr { td { (text(u, "handle")) } td {
                    @if u["id"] == auth.user { (text(u, "role")) } @else {
                        form method="post"
                            action={ "/api/v1/admin/users/" (u["id"]) "?_method=PATCH&back=/admin" } {
                            (choose("role", "Role", &ROLES, text(u, "role")))
                            (choose("status", "Status", &["active", "suspended"], text(u, "status")))
                            button { "Save" }
                        }
                    }
                } } } }
            }
        }
        p { a href="/api/v1/admin/transparency" { "Transparency report (CSV)" } " · "
            a href="/api/v1/mod/audit" { "Audit log (JSON)" } }
    };
    Ok(page(&app, Some(&auth), "Administration", body).into_response())
}
