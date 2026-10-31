use crate::account::{Register, authenticate, create_user, insert_user};
use crate::approval::{pending_device, user_code};
use crate::auth::Auth;
use crate::creds::{self, check_handle, check_password, digest};
use crate::db::now;
use crate::error::{Problem, Res};
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
        .route("/account", get(crate::account_page::account))
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
    let pending = match user_code(&code) {
        Ok(key) => app.db.run(move |c| pending_device(c, &key)).await.ok(),
        Err(_) => None,
    };
    let at = |v: &Value| {
        chrono::DateTime::from_timestamp_millis(v.as_i64().unwrap_or_default()).unwrap_or_default()
    };
    let body = html! {
        p { strong { "Approve only a code that your own new device shows on its screen." } " "
            "Someone who sends you a code or a link is trying to get into your account." }
        @if let Some(d) = &pending {
            dl {
                dt { "Code" } dd { code { (code) } }
                dt { "Platform" } dd { (text(d, "platform")) }
                dt { "Software" } dd { (d["client"].as_str().filter(|c| !c.is_empty()).unwrap_or("not given")) }
                dt { "Requested" } dd { (at(&d["requested_at"]).format("%Y-%m-%d %H:%M UTC")) }
            }
            form.stack method="post" action="/api/v1/auth/device/approve?back=/account" {
                input type="hidden" name="user_code" value=(code);
                div {
                    button name="approve" value="true" { "This is my device: approve" } " "
                    button name="approve" value="false" { "Deny" }
                }
            }
        } @else {
            @if !code.is_empty() { p { "No device waits for this code: it is wrong, used or expired." } }
            form.stack method="get" action="/link" {
                label { "Code shown on the new device" input name="code" value=(code) autocomplete="off" required; }
                button { "Look up" }
            }
        }
    };
    page(&app, Some(&auth), "Link a device", body).into_response()
}
