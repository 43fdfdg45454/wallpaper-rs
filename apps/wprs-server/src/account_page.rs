use crate::St;
use crate::account::profile;
use crate::auth::Auth;
use crate::error::{Problem, Res};
use crate::me::listing;
use crate::web::{page, text};
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde_json::Value;

fn remove(path: String) -> Markup {
    html! { form method="post" action={ (path) "?_method=DELETE&back=/account" } { button { "Remove" } } }
}

pub async fn account(State(app): St, auth: Option<Auth>) -> Res<Response> {
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
            @if app.repo.online() {
                p { "Public minisign keys that sign your packages. Clients trust the key that signed the first \
                    version they installed of each package, so a new key needs a rotation signed by the old \
                    one: " code { "wprs-ctl pack rotate-key OLD.key NEW.pub --author " (crate::repo::author(auth.user)) } }
            } @else {
                p { "Public minisign keys that sign your packages. The repository root must list them too." }
            }
            table { tbody { @for k in &keys { tr {
                td { code { (text(k, "fingerprint")) } }
                td { (remove(format!("/api/v1/me/author-keys/{}", text(k, "fingerprint")))) }
            } } } }
            form method="post" action="/api/v1/me/author-keys?back=/account" {
                label { "Public key" input name="public_key" required; } button { "Add key" }
            }
            @if app.repo.online() {
                form.stack method="post" action="/api/v1/me/author-keys/rotate?back=/account" {
                    label { "Old key fingerprint" input name="from" required pattern="[0-9a-f]{64}"; }
                    label { "New public key" input name="public_key" required; }
                    label { "Rotation signature" textarea name="signature" rows="4" required {} }
                    button { "Rotate to the new key" }
                }
            }
        }
        section.card { h2 { "Password and data" }
            @if app.cfg.oidc.is_some() {
                p { a href="/api/v1/auth/oidc/start?link=true" { "Link single sign-on" } }
            }
            p.muted { "A new password signs out every other session and deletes your access tokens. Your "
                "devices stay linked unless you also sign them out." }
            form method="post" action="/api/v1/me/password?back=/account" {
                label { "Current password"
                    input type="password" name="current" autocomplete="current-password"; }
                label { "New password"
                    input type="password" name="new" autocomplete="new-password" minlength="10" required; }
                label { input type="checkbox" name="devices" value="true"; "Also sign out every device" }
                p.muted { "Signed-out devices have to be linked again and to get the sync key from a device "
                    "that still holds it; without one, or a recovery passphrase, the encrypted sync data "
                    "has to be reset." }
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
