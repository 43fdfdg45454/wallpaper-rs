use crate::account::{Register, authenticate, create_user, insert_user, profile};
use crate::auth::Auth;
use crate::creds::{self, check_handle, check_password, digest};
use crate::db::now;
use crate::error::{Problem, Res};
use crate::me::listing;
use crate::net::{Client, LOGIN};
use crate::web::{page, text};
use crate::{App, Router, St};
use axum::extract::{Form, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use rusqlite::params;
use serde::Deserialize;
use serde_json::Value;

pub fn routes() -> Router {
    Router::new()
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/register", get(register_page).post(register))
        .route("/setup", get(setup_page).post(setup))
        .route("/link", get(link))
        .route("/account", get(account))
}

#[derive(Deserialize, Default)]
pub struct Next {
    #[serde(default)]
    next: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    invite: String,
}

pub fn local(path: &str) -> &str {
    let plain = path.bytes().all(|b| b.is_ascii_graphic() && b != b'\\');
    if plain && path.starts_with('/') && !path.starts_with("//") { path } else { "/account" }
}

async fn start_session(app: &App, user: i64, next: &str) -> Res<Response> {
    let value = app.db.run(move |c| creds::session(c, user)).await?;
    let cookie = creds::cookie(&app.cfg, &value, 30 * 86400);
    Ok(([(header::SET_COOKIE, cookie)], Redirect::to(local(next))).into_response())
}

async fn login_page(State(app): St, Query(n): Query<Next>) -> Markup {
    let next = local(&n.next);
    let body = html! {
        form.stack method="post" action="/login" {
            input type="hidden" name="next" value=(next);
            label { "Handle" input name="handle" autocomplete="username" required; }
            label { "Password"
                input type="password" name="password" autocomplete="current-password" required; }
            button { "Sign in" }
        }
        @if app.cfg.oidc.is_some() {
            p { a href={ "/api/v1/auth/oidc/start?next=" (next) } { "Sign in with single sign-on" } }
        }
        p { "No account yet? " a href="/register" { "Register" } }
    };
    page(&app, None, "Sign in", body)
}

#[derive(Deserialize)]
struct Credentials {
    handle: String,
    password: String,
    #[serde(default)]
    next: String,
}

async fn login(State(app): St, client: Client, Form(f): Form<Credentials>) -> Res<Response> {
    let user = authenticate(&app, &client, &f.handle, f.password).await?;
    start_session(&app, user, &f.next).await
}

async fn logout(State(app): St, auth: Option<Auth>) -> Res<Response> {
    let session = auth.and_then(|a| a.session);
    app.db.run(move |c| Ok(c.execute("DELETE FROM sessions WHERE id_hash = ?1", [session])?)).await?;
    Ok(([(header::SET_COOKIE, creds::cookie(&app.cfg, "", 0))], Redirect::to("/")).into_response())
}

async fn register_page(State(app): St, Query(n): Query<Next>) -> Markup {
    let body = html! {
        form.stack method="post" action="/register" {
            label { "Handle"
                input name="handle" autocomplete="username" required minlength="3" maxlength="32"; }
            label { "Password (10 characters or more)"
                input type="password" name="password" autocomplete="new-password" required minlength="10"; }
            label { "Invitation code" input name="invite" value=(n.invite) autocomplete="off"; }
            button { "Create account" }
        }
        p { "By registering you accept the " a href="/legal/terms" { "terms" } " and the "
            a href="/legal/privacy" { "privacy notice" } "." }
    };
    page(&app, None, "Register", body)
}

async fn register(State(app): St, client: Client, Form(r): Form<Register>) -> Res<Response> {
    let user = create_user(&app, &client, r).await?;
    start_session(&app, user, "/account").await
}

async fn setup_page(State(app): St) -> Markup {
    let body = html! {
        p { "Enter the one-time setup token printed in the server log, or made with "
            code { "wprs-server setup-token" } "." }
        form.stack method="post" action="/setup" {
            label { "Setup token" input name="token" autocomplete="off" required; }
            label { "Handle" input name="handle" autocomplete="username" required; }
            label { "Password (10 characters or more)"
                input type="password" name="password" autocomplete="new-password" required minlength="10"; }
            button { "Create administrator" }
        }
    };
    page(&app, None, "Create the first administrator", body)
}

#[derive(Deserialize)]
struct Setup {
    token: String,
    handle: String,
    password: String,
}

async fn setup(State(app): St, client: Client, Form(s): Form<Setup>) -> Res<Response> {
    app.limits.take(&LOGIN, &client.key())?;
    check_handle(&s.handle)?;
    check_password(&s.handle, &s.password)?;
    let (hash, token, quota) =
        (creds::hash(&app, s.password).await?, digest(s.token.trim()), app.cfg.user_quota);
    let user = app.db.run(move |c| {
        let tx = c.transaction()?;
        let used = tx
            .execute("DELETE FROM setup_tokens WHERE hash = ?1 AND expires_at > ?2", params![token, now()])?;
        if used == 0 {
            return Err(Problem::forbidden("the setup token is wrong, used or expired"));
        }
        let id = insert_user(&tx, &s.handle, Some(&hash), "admin", quota)?;
        tx.execute("DELETE FROM setup_tokens", [])?;
        tx.commit()?;
        Ok(id)
    });
    let user = user.await?;
    start_session(&app, user, "/admin").await
}

async fn link(State(app): St, auth: Option<Auth>, Query(n): Query<Next>) -> Response {
    let code: String = n.code.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let Some(auth) = auth else {
        return Redirect::to(&format!("/login?next=/link%3Fcode%3D{code}")).into_response();
    };
    let body = html! {
        p { "Compare the code with the one on the new device, then approve it. "
            "It gets its own token; revoke it any time on your account page." }
        form.stack method="post" action="/api/v1/auth/device/approve?back=/account" {
            label { "Code" input name="user_code" value=(code) autocomplete="off" required; }
            div {
                button name="approve" value="true" { "Approve" } " "
                button name="approve" value="false" { "Deny" }
            }
        }
    };
    page(&app, Some(&auth), "Link a device", body).into_response()
}

fn remove(path: String) -> Markup {
    html! { form method="post" action={ (path) "?_method=DELETE&back=/account" } { button { "Remove" } } }
}

async fn account(State(app): St, auth: Option<Auth>) -> Res<Response> {
    let Some(auth) = auth else { return Ok(Redirect::to("/login?next=/account").into_response()) };
    let user = auth.user;
    let data = app.db.run(move |c| {
        let lists = ["versions", "uploads", "devices", "tokens", "keys"].map(|w| listing(c, user, w));
        Ok((profile(c, user)?, lists.into_iter().collect::<Res<Vec<_>>>()?))
    });
    let (me, lists) = data.await?;
    let [versions, uploads, devices, tokens, keys] =
        <[Vec<Value>; 5]>::try_from(lists).map_err(|_| Problem::internal("account lists"))?;
    let body = html! {
        p { "Signed in as " strong { (auth.handle) } " (" (auth.role.name()) "). Storage used: "
            (me["used_bytes"]) " of " (me["quota_bytes"]) " bytes." }
        section.card { h2 { "Upload a wallpaper package" }
            form method="post" action="/upload?back=/account" enctype="multipart/form-data" {
                label { ".wpak file" input type="file" name="package" accept=".wpak" required; }
                button { "Upload" }
            }
            table {
                thead { tr { th { "Package" } th { "Version" } th { "State" } th { "Moderation" } } }
                tbody {
                    @for v in &versions { tr {
                        td { (text(v, "title")) br; code { (text(v, "package")) } }
                        td { (text(v, "version")) } td { (text(v, "state")) } td { (text(v, "statement")) }
                    } }
                    @for u in &uploads { tr {
                        td { "Upload " (text(u, "id")) } td {}
                        td { (text(u, "state")) } td { (text(u, "errors")) }
                    } }
                }
            }
        }
        section.card { h2 { "Devices" }
            p { a href="/link" { "Link a new device with its code" } }
            table { tbody { @for d in &devices { tr {
                td { (text(d, "name")) } td { (text(d, "platform")) }
                td { (remove(format!("/api/v1/me/devices/{}", text(d, "id")))) }
            } } } }
        }
        section.card { h2 { "Access tokens" }
            table { tbody { @for t in &tokens { tr {
                td { (text(t, "name")) } td { code { (text(t, "scopes")) } }
                td { @if t["device_id"].is_null() { (remove(format!("/api/v1/me/tokens/{}", t["id"]))) } }
            } } } }
            form method="post" action="/api/v1/me/tokens?back=/account" {
                label { "Name" input name="name" required maxlength="64"; }
                label { "Scopes" input name="scopes" value="catalog:read upload" required; }
                label { "Days" input type="number" name="days" min="1" max="365" value="30"; }
                button { "Create token" }
            }
        }
        section.card { h2 { "Author keys" }
            p { "Public minisign keys that sign your packages. The repository root must list them too." }
            table { tbody { @for k in &keys { tr {
                td { code { (text(k, "fingerprint")) } }
                td { (remove(format!("/api/v1/me/author-keys/{}", text(k, "fingerprint")))) }
            } } } }
            form method="post" action="/api/v1/me/author-keys?back=/account" {
                label { "Public key" input name="public_key" required; } button { "Add key" }
            }
        }
        section.card { h2 { "Password and data" }
            @if app.cfg.oidc.is_some() {
                p { a href="/api/v1/auth/oidc/start?link=true" { "Link single sign-on" } }
            }
            form method="post" action="/api/v1/me/password?back=/account" {
                label { "Current password"
                    input type="password" name="current" autocomplete="current-password"; }
                label { "New password"
                    input type="password" name="new" autocomplete="new-password" minlength="10" required; }
                button { "Change password" }
            }
            p { a href="/api/v1/me/export" { "Download all your data (JSON)" } }
            form method="post" action="/api/v1/me?_method=DELETE&back=/" {
                label { "Password" input type="password" name="password" autocomplete="current-password"; }
                button { "Delete my account" }
            }
        }
    };
    Ok(page(&app, Some(&auth), "Your account", body).into_response())
}
