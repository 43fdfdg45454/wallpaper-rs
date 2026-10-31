use super::{Web, folders, script, views};
use crate::{Error, pack};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use wprs_format::page::{csp, decoded, mime};

const API: &str = ":wprs/api.js";
const GUARD: &str = ":wprs/guard.js";
const WORKER: &str = ":wprs/worker.js";
const TAG: &str = "<script src=\"/:wprs/api.js\"></script>";
const OWN: &str = "'self' data: blob:";

pub(super) fn inject(html: &str) -> String {
    let start = html.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    let skipped = html.len() - start.len();
    let typed = start.get(..9).is_some_and(|head| head.eq_ignore_ascii_case("<!doctype"));
    let at = typed.then(|| start.find('>').map(|end| skipped + end + 1)).flatten().unwrap_or(0);
    format!("{}{TAG}{}", &html[..at], &html[at..])
}

fn extracted(web: &Web, path: &str) -> Result<PathBuf, Error> {
    let file = pack::folder(&web.wpak)?.join("page").join(path);
    if file.is_file() {
        return Ok(file);
    }
    let mut package = wprs_format::open(&web.wpak, &wprs_format::Limits::default())?;
    let mut bytes = package.read_file(path)?;
    if mime(path) == "text/html" {
        bytes = inject(&String::from_utf8_lossy(&bytes)).into_bytes();
    }
    let parent = file.parent().ok_or(Error::Unprepared)?;
    fs::create_dir_all(parent)?;
    let mut part = tempfile::NamedTempFile::new_in(parent)?;
    part.write_all(&bytes)?;
    part.persist(&file).map_err(|e| e.error)?;
    Ok(file)
}

fn origin(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let end = rest.find(['/', '?', '#']).map_or(url.len(), |at| url.len() - rest.len() + at);
    &url[..end]
}

pub(crate) fn serve(handle: i64, url: &str) -> Result<Value, Error> {
    let mut views = views();
    let Some(view) = views.get_mut(&handle) else { return Ok(json!({})) };
    let pack = view.pack.clone();
    let Some(web) = pack.web.as_ref() else { return Ok(json!({})) };
    let network = if view.privacy.network { web.network.clone() } else { Vec::new() };
    let reply = |kind: &str, key: &str, value: Value| json!({"mime": kind, "csp": csp(OWN, "'self'", &network), key: value});
    let own = format!("https://{}/", view.host);
    let Some(rest) = url.strip_prefix(&own) else {
        let allowed = network.iter().any(|o| o == origin(url));
        return Ok(if allowed { json!({"network": true}) } else { json!({}) });
    };
    let bare = url.split(['?', '#']).next().unwrap_or_default();
    let Some(path) = decoded(rest.split(['?', '#']).next().unwrap_or_default()) else { return Ok(json!({})) };
    let script = match path.as_str() {
        API => Some(script::source(view)),
        GUARD => Some(script::guard(view)),
        WORKER => rest.split_once('?').and_then(|(_, query)| script::worker(view, query)),
        _ => None,
    };
    if let Some(text) = script {
        return Ok(reply("text/javascript", "text", json!(text)));
    }
    if let Some(file) = folders::user(&path).and(view.files.get(bare)) {
        return Ok(reply(mime(&file.to_string_lossy()), "path", json!(file)));
    }
    let path = if path.is_empty() { web.entry.clone() } else { path };
    if !web.entries.contains(&path) {
        return Ok(json!({}));
    }
    drop(views);
    Ok(reply(mime(&path), "path", json!(extracted(web, &path)?)))
}
