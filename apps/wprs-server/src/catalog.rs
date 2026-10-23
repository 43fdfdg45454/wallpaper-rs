use crate::db::rows;
use crate::error::{Problem, Res};
use crate::web::text;
use crate::{Reply, Router, St};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD as B64;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

const SELECT: &str = "SELECT v.id AS row, v.package_id AS id, v.version, v.kind, v.title, v.description, \
                      v.license, v.tags, v.authors, v.size, v.sha256, v.published_at, u.handle AS owner, \
                      v.content, v.photosensitive, v.facets \
                      FROM versions v JOIN packages p ON p.id = v.package_id \
                      LEFT JOIN users u ON u.id = p.owner_id";
const NEWEST: &str = " WHERE v.state = 'published' AND v.id = (SELECT w.id FROM versions w \
                      WHERE w.package_id = v.package_id AND w.state = 'published' \
                      ORDER BY w.published_at DESC, w.id DESC LIMIT 1) \
                      AND (?1 = '' OR (?1 = 'm' AND v.id IN (SELECT rowid FROM search WHERE search MATCH ?2)) \
                      OR (?1 = 't' AND v.tags LIKE ?2 ESCAPE '!')) \
                      AND (?3 = '' OR v.kind = ?3) AND (v.published_at, v.id) < (?4, ?5) \
                      AND (?7 = '' OR v.content = ?7 OR (?7 = 'unrated' AND v.content IS NULL)) \
                      AND (?8 = '' OR EXISTS (SELECT 1 FROM json_each(v.facets) WHERE value = ?8)) \
                      ORDER BY v.published_at DESC, v.id DESC LIMIT ?6";

pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/catalog", get(list))
        .route("/api/v1/packages/{id}", get(package))
        .route("/api/v1/packages/{id}/versions", get(versions))
}

#[derive(Deserialize, Default)]
pub struct Search {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub cursor: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub facet: String,
    pub limit: Option<u32>,
}

fn shape(mut item: Value) -> Value {
    for key in ["tags", "authors", "facets"] {
        item[key] = serde_json::from_str(item[key].as_str().unwrap_or("[]")).unwrap_or_default();
    }
    item["photosensitive"] = item["photosensitive"].as_i64().map(|f| f != 0).into();
    let (id, version, sha) = (text(&item, "id"), text(&item, "version"), text(&item, "sha256"));
    let (thumbnail, download) = (format!("/media/{sha}.jpg"), format!("/repo/packages/{id}/{version}.wpak"));
    (item["thumbnail"], item["download"]) = (thumbnail.into(), download.into());
    item.as_object_mut().map(|fields| fields.remove("row"));
    item
}

fn pattern(q: &str) -> (&'static str, String) {
    let long: Vec<String> = q
        .split_whitespace()
        .filter(|term| term.chars().count() >= 3)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    match q.trim() {
        "" => ("", String::new()),
        _ if !long.is_empty() => ("m", long.join(" ")),
        short => ("t", format!("%\"{}%", short.replace('!', "!!").replace('%', "!%").replace('_', "!_"))),
    }
}

pub fn search(c: &Connection, s: &Search) -> Res<(Vec<Value>, Option<String>)> {
    let limit = s.limit.unwrap_or(24).clamp(1, 100) as usize;
    let cursor = B64.decode(&s.cursor).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
    let after = cursor.split_once(':').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)));
    let (at, row): (i64, i64) = after.unwrap_or((i64::MAX, i64::MAX));
    let (mode, pattern) = pattern(&s.q);
    let values = params![mode, pattern, s.kind, at, row, limit as i64 + 1, s.content, s.facet];
    let mut found = rows(c, &format!("{SELECT}{NEWEST}"), values)?;
    let more = found.len() > limit;
    found.truncate(limit);
    let next =
        found.last().filter(|_| more).map(|i| B64.encode(format!("{}:{}", i["published_at"], i["row"])));
    Ok((found.into_iter().map(shape).collect(), next))
}

pub fn details(c: &Connection, id: &str) -> Res<(Value, Vec<Value>)> {
    let sql = format!(
        "{SELECT} WHERE v.package_id = ?1 AND v.state = 'published' ORDER BY v.published_at DESC, v.id DESC"
    );
    let versions: Vec<Value> = rows(c, &sql, [id])?.into_iter().map(shape).collect();
    let latest = versions.first().cloned().ok_or_else(|| Problem::missing("package"))?;
    Ok((latest, versions))
}

async fn list(State(app): St, Query(s): Query<Search>) -> Res<impl IntoResponse> {
    let encode = |text: &str| utf8_percent_encode(text, NON_ALPHANUMERIC).to_string();
    let filters = [
        ("q", encode(&s.q)),
        ("kind", encode(&s.kind)),
        ("content", encode(&s.content)),
        ("facet", encode(&s.facet)),
        ("limit", s.limit.map(|l| l.to_string()).unwrap_or_default()),
    ];
    let (items, next) = app.db.run(move |c| search(c, &s)).await?;
    let link = next.as_ref().map(|cursor| {
        let rest: String =
            filters.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("&{k}={v}")).collect();
        format!("</api/v1/catalog?cursor={cursor}{rest}>; rel=\"next\"")
    });
    Ok((link.map(|l| [(header::LINK, l)]), Json(json!({"items": items, "next_cursor": next}))))
}

async fn package(State(app): St, Path(id): Path<String>) -> Reply {
    let (mut latest, versions) = app.db.run(move |c| details(c, &id)).await?;
    latest["versions"] = versions.iter().map(|v| v["version"].clone()).collect();
    Ok(Json(latest))
}

async fn versions(State(app): St, Path(id): Path<String>) -> Reply {
    let (_, versions) = app.db.run(move |c| details(c, &id)).await?;
    Ok(Json(json!({"items": versions})))
}
