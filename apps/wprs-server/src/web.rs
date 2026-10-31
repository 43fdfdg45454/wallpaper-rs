use crate::auth::Auth;
use crate::catalog::{Search, details, search};
use crate::db::one;
use crate::error::{Problem, Res};
use crate::layout::{STYLE, preview};
pub use crate::layout::{layout, page};
use crate::{Router, St};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, PreEscaped, html};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;
use wprs_format::markup;

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

const FACETS: [&str; 8] =
    ["audio", "pointer", "sound", "customizable", "localized", "photosensitive", "mature", "4k"];

async fn home(State(app): St, auth: Option<Auth>, Query(s): Query<Search>) -> Res<Markup> {
    let (q, kind, rating, facet) = (s.q.clone(), s.kind.clone(), s.content.clone(), s.facet.clone());
    let (items, next) = app.db.run(move |c| search(c, &s)).await?;
    let encode = |text: &str| utf8_percent_encode(text, NON_ALPHANUMERIC).to_string();
    let body = html! {
        form role="search" method="get" action="/" {
            label { "Search" input type="search" name="q" value=(q); }
            label { "Type" select name="kind" {
                option value="" { "Any" }
                @for k in ["image", "video", "scene", "web"] { option value=(k) selected[kind == k] { (k) } }
            } }
            label { "Rating" select name="content" {
                option value="" { "Any" }
                @for r in ["general", "mature", "unrated"] { option value=(r) selected[rating == r] { (r) } }
            } }
            label { "With" select name="facet" {
                option value="" { "Anything" }
                @for f in FACETS { option value=(f) selected[facet == f] { (f) } }
            } }
            button { "Search" }
        }
        @if items.is_empty() { p { "Nothing found." } }
        ul.grid { @for i in &items { li {
            (preview(text(i, "thumbnail"), "", 320, i["content"] == "mature"))
            a href={ "/packages/" (text(i, "id")) } { (text(i, "title")) }
            br;
            small {
                (text(i, "kind")) " · " (text(i, "license"))
                @if let Some(o) = i["owner"].as_str() { " · " (o) }
            }
        } } }
        @if let Some(next) = next {
            p { a rel="next" href={ "/?cursor=" (next) "&q=" (encode(&q)) "&kind=" (encode(&kind))
                "&content=" (encode(&rating)) "&facet=" (encode(&facet)) } {
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
    let render = if latest["content"].is_null() { markup::verbatim } else { markup::html };
    let description = render(text(&latest, "description"));
    let body = html! {
        (preview(text(&latest, "thumbnail"), &format!("Preview of {title}"), 640, latest["content"] == "mature"))
        @if let Some(reason) = latest["hidden"].as_str() {
            p.warning { "The moderators took this wallpaper out of the catalog: " (reason) }
        }
        @if latest["photosensitive"] == true {
            p.warning { "This wallpaper has flashing lights or patterns that can affect people with \
                photosensitive epilepsy." }
        }
        (PreEscaped(description))
        dl {
            dt { "Package" } dd { code { (id) } }
            dt { "Type" } dd { (text(&latest, "kind")) }
            dt { "License" } dd { (text(&latest, "license")) }
            dt { "Authors" } dd { (joined(&latest["authors"])) }
            dt { "Tags" } dd { (joined(&latest["tags"])) }
            dt { "Rating" } dd { (latest["content"].as_str().unwrap_or("unrated")) }
            dt { "Facets" } dd { (joined(&latest["facets"])) }
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
