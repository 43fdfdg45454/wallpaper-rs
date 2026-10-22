use crate::auth::{Auth, Role};
use crate::catalog::{Search, details, search};
use crate::db::one;
use crate::error::{Problem, Res};
use crate::{App, Router, St};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

const STYLE: &str = "
:root{color-scheme:light dark;--fg:#1b1b1f;--bg:#fafafa;--muted:#55555f;--line:#c8c8d0;
--accent:#1d4ed8;--card:#fff}
@media (prefers-color-scheme:dark){:root{--fg:#ececf1;--bg:#141418;--muted:#b4b4c0;--line:#3a3a44;
--accent:#93b4ff;--card:#1e1e24}}
*{box-sizing:border-box}body{margin:0;font:16px/1.5 system-ui,sans-serif;color:var(--fg);background:var(--bg)}
a{color:var(--accent)}
a:focus,button:focus,input:focus,select:focus,textarea:focus{outline:3px solid var(--accent);
outline-offset:2px}
.skip{position:absolute;left:-999px}
.skip:focus{left:8px;top:8px;background:var(--card);padding:8px}
header,footer{border-bottom:1px solid var(--line);padding:8px 16px}
footer{border-top:1px solid var(--line);border-bottom:0;margin-top:32px}
nav{display:flex;flex-wrap:wrap;gap:16px;align-items:center}main{max-width:1100px;margin:0 auto;padding:16px}
form{display:flex;flex-wrap:wrap;gap:8px;align-items:end;margin:8px 0}
form.stack{flex-direction:column;align-items:stretch;max-width:560px}
label{display:flex;flex-direction:column;gap:4px;font-weight:600}
input,select,textarea,button{font:inherit;padding:6px 10px;border:1px solid var(--line);border-radius:6px;
background:var(--card);color:var(--fg)}button{cursor:pointer;font-weight:600}
ul.grid{list-style:none;padding:0;display:grid;gap:16px;
grid-template-columns:repeat(auto-fill,minmax(220px,1fr))}
ul.grid li,section.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:8px}
img{max-width:100%;height:auto;border-radius:6px}table{border-collapse:collapse;width:100%}
th,td{text-align:left;border-bottom:1px solid var(--line);padding:6px;vertical-align:top}
.muted,small{color:var(--muted)}dl{display:grid;grid-template-columns:max-content 1fr;gap:4px 16px}
dd{margin:0;overflow-wrap:anywhere}
";

pub fn routes() -> Router {
    let style = || async {
        ([(header::CONTENT_TYPE, "text/css"), (header::CACHE_CONTROL, "public, max-age=3600")], STYLE)
    };
    Router::new()
        .route("/", get(home))
        .route("/packages/{id}", get(package))
        .route("/media/{file}", get(media))
        .route("/static/style.css", get(style))
}

pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn joined(value: &Value) -> String {
    value.as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")
}

pub fn page(app: &App, auth: Option<&Auth>, title: &str, body: Markup) -> Markup {
    let account = html! {
        @if let Some(a) = auth {
            a href="/account" { "Account (" (a.handle) ")" }
            @if a.role >= Role::Moderator { a href="/mod" { "Moderation" } }
            @if a.role >= Role::Admin { a href="/admin" { "Administration" } }
            form method="post" action="/logout" { button { "Sign out" } }
        } @else {
            a href="/login" { "Sign in" }
        }
    };
    layout(app, Some(account), title, body)
}

pub fn layout(app: &App, account: Option<Markup>, title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (app.cfg.name) }
                link rel="stylesheet" href="/static/style.css";
            }
            body {
                a.skip href="#main" { "Skip to content" }
                header {
                    nav aria-label="Site" {
                        a href="/" { strong { (app.cfg.name) } }
                        a href="/" { "Catalog" }
                        @if let Some(a) = account { (a) }
                    }
                }
                main id="main" { h1 { (title) } (body) }
                footer {
                    nav aria-label="Legal" {
                        a href="/legal" { "Legal notice" }
                        a href="/legal/terms" { "Terms" }
                        a href="/legal/privacy" { "Privacy" }
                        a href="/legal/content" { "Content policy" }
                        a href="/legal/notice" { "Report content" }
                    }
                }
            }
        }
    }
}

async fn home(State(app): St, auth: Option<Auth>, Query(s): Query<Search>) -> Res<Markup> {
    let (q, kind) = (s.q.clone(), s.kind.clone());
    let (items, next) = app.db.run(move |c| search(c, &s)).await?;
    let encode = |text: &str| utf8_percent_encode(text, NON_ALPHANUMERIC).to_string();
    let body = html! {
        form role="search" method="get" action="/" {
            label { "Search" input type="search" name="q" value=(q); }
            label { "Type" select name="kind" {
                option value="" { "Any" }
                @for k in ["image", "video", "scene"] { option value=(k) selected[kind == k] { (k) } }
            } }
            button { "Search" }
        }
        @if items.is_empty() { p { "Nothing found." } }
        ul.grid { @for i in &items { li {
            a href={ "/packages/" (text(i, "id")) } {
                img src=(text(i, "thumbnail")) alt="" width="320" loading="lazy"; br; (text(i, "title"))
            }
            br;
            small {
                (text(i, "kind")) " · " (text(i, "license"))
                @if let Some(o) = i["owner"].as_str() { " · " (o) }
            }
        } } }
        @if let Some(next) = next {
            p { a rel="next" href={ "/?cursor=" (next) "&q=" (encode(&q)) "&kind=" (encode(&kind)) } {
                "More wallpapers"
            } }
        }
    };
    Ok(page(&app, auth.as_ref(), "Wallpapers", body))
}

async fn package(State(app): St, auth: Option<Auth>, Path(id): Path<String>) -> Res<Markup> {
    let key = id.clone();
    let (latest, versions) = app.db.run(move |c| details(c, &key)).await?;
    let (title, owner) = (text(&latest, "title"), latest["owner"].as_str().unwrap_or("a deleted account"));
    let body = html! {
        img src=(text(&latest, "thumbnail")) alt={ "Preview of " (title) } width="640";
        p { (text(&latest, "description")) }
        dl {
            dt { "Package" } dd { code { (id) } }
            dt { "Type" } dd { (text(&latest, "kind")) }
            dt { "License" } dd { (text(&latest, "license")) }
            dt { "Authors" } dd { (joined(&latest["authors"])) }
            dt { "Tags" } dd { (joined(&latest["tags"])) }
            dt { "Uploaded by" } dd { (owner) }
            dt { "SHA-256" } dd { code { (text(&latest, "sha256")) } }
        }
        p {
            a href=(text(&latest, "download")) { "Download version " (text(&latest, "version")) }
            " · or install it with a client that verifies the signed repository."
        }
        h2 { "Versions" }
        ul { @for v in &versions { li { a href=(text(v, "download")) { (text(v, "version")) } } } }
        h2 { "Report this wallpaper" }
        form.stack method="post" action={ "/api/v1/packages/" (id) "/reports?back=/packages/" (id) } {
            label { "Reason" select name="category" {
                @for c in ["copyright", "illegal", "malware", "mature", "spam", "other"] {
                    option value=(c) { (c) }
                }
            } }
            label { "Details" textarea name="details" rows="3" maxlength="4000" {} }
            button { "Send report" }
        }
        p.muted {
            "For legal notices with your contact details use the "
            a href="/legal/notice" { "notice form" } "."
        }
    };
    Ok(page(&app, auth.as_ref(), title, body))
}

async fn media(State(app): St, Path(file): Path<String>) -> Res<Response> {
    let lower =
        |s: &&str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let sha = file.strip_suffix(".jpg").filter(lower).ok_or_else(|| Problem::missing("image"))?.to_string();
    let (key, sql) =
        (sha.clone(), "SELECT 1 FROM versions WHERE sha256 = ?1 AND state IN ('pending', 'published')");
    app.db.run(move |c| one::<i64>(c, sql, [key])).await?.ok_or_else(|| Problem::missing("image"))?;
    let path = app.cfg.data.join("thumbs").join(format!("{sha}.jpg"));
    let file = tokio::fs::File::open(path).await.map_err(|_| Problem::missing("image"))?;
    let headers = [
        (header::CONTENT_TYPE, "image/jpeg"),
        (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
    ];
    Ok((headers, Body::from_stream(tokio_util::io::ReaderStream::new(file))).into_response())
}
