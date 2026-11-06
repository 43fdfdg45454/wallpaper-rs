use crate::findings::{Problem, problem};
use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_format::{MANIFEST, SIGNATURE};

pub const PAGE_STARTERS: [&str; 1] = ["web"];
const PAGE: [(&str, &str); 2] = [
    ("index.html", include_str!("../starters/web/index.html")),
    ("app.js", include_str!("../starters/web/app.js")),
];

pub fn scened(doc: &Value) -> bool {
    matches!(doc["manifest"]["kind"].as_str(), None | Some("scene" | "assets"))
}

pub fn kind(doc: &Value) -> &str {
    doc["manifest"]["kind"].as_str().unwrap_or("scene")
}

pub(crate) fn manifest(name: &str, base: Value) -> Option<Value> {
    let mut manifest = base;
    match name {
        "web" => {
            manifest["kind"] = json!("web");
            manifest["entry"] = json!("index.html");
            manifest["inputs"] = json!([]);
            manifest["web"] = json!({"background": "#101828"});
            manifest["properties"] =
                json!({"color": {"type": "color", "label": "@wprs.color", "default": "#f2f5ff"}});
        }
        _ => return None,
    }
    Some(json!({"manifest": manifest}))
}

pub(crate) fn seed(dir: &Path, doc: &Value) -> Result<()> {
    if kind(doc) != "web" || dir.join(doc["manifest"]["entry"].as_str().unwrap_or("index.html")).exists() {
        return Ok(());
    }
    for (name, text) in PAGE {
        crate::session::write(&dir.join(name), text, 0o644)?;
    }
    Ok(())
}

fn role(doc: &Value, path: &str) -> &'static str {
    let m = &doc["manifest"];
    let is = |v: &Value| v.as_str() == Some(path);
    match () {
        _ if path == MANIFEST => "manifest",
        _ if path == SIGNATURE => "signature",
        _ if is(&m["entry"]) => "entry",
        _ if is(&m["web"]["preload"]) => "preload",
        _ if is(&m["previews"]["thumbnail"]) || is(&m["previews"]["animated"]) => "preview",
        _ if wprs_format::locale_file(path) => "locale",
        _ => "file",
    }
}

pub fn kind_of(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "page",
        "js" | "mjs" => "script",
        "css" => "style",
        "wasm" => "wasm",
        "json" => "data",
        _ => crate::layers::asset_kind(path),
    }
}

pub fn files(dir: &Path, doc: &Value) -> Vec<Value> {
    let found = crate::assets::files(dir).into_iter().filter(|(p, _)| !p.starts_with('.'));
    found
        .map(|(path, size)| json!({"path": path, "size": size, "kind": kind_of(&path), "role": role(doc, &path)}))
        .collect()
}

pub fn scripts(dir: &Path) -> Vec<String> {
    let found = crate::assets::files(dir).into_iter().map(|(p, _)| p);
    found.filter(|p| matches!(kind_of(p), "script")).collect()
}

pub fn review(doc: &Value, dir: Option<&Path>) -> Vec<Problem> {
    let mut out = Vec::new();
    let m = &doc["manifest"];
    let Some(dir) = dir else { return out };
    let missing = |field: &str, path: &str| {
        problem(field, said("editor.not_in_folder", "%1 is not in the folder", &[&path]))
    };
    if let Some(entry) = m["entry"].as_str().filter(|e| !dir.join(e).is_file()) {
        out.push(missing("/manifest/entry", entry));
    }
    if let Some(preload) = m["web"]["preload"].as_str().filter(|e| !dir.join(e).is_file()) {
        out.push(missing("/manifest/web/preload", preload));
    }
    out
}

pub fn copy_in(dir: &Path, file: &Path, to: Option<&str>) -> Result<String> {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let path = match to.map(|t| t.trim_matches('/')).filter(|t| !t.is_empty()) {
        Some(folder) => format!("{folder}/{name}"),
        None => name,
    };
    wprs_format::check_path(&path)?;
    anyhow::ensure!(
        path != MANIFEST && path != SIGNATURE,
        said("editor.reserved_name", "%1 is a name the package keeps for itself", &[&path])
    );
    let target = dir.join(&path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(file, &target)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?;
    Ok(path)
}
