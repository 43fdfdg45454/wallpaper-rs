use crate::project::Project;
use crate::shelf::{Pick, files, fragment};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wprs_core::message::said;
use wprs_format::{SCHEMA, check_path, valid_id};

#[derive(Debug, Clone, Default)]
pub struct Offer {
    pub id: String,
    pub title: String,
    pub license: String,
    pub attribution: String,
    pub thumbnail: Option<PathBuf>,
    pub thumbnails: BTreeMap<String, PathBuf>,
    pub titles: BTreeMap<String, String>,
}

fn picture(file: &Path, name: &str, pictures: &mut Vec<(String, Vec<u8>)>) -> Result<String> {
    let data = fs::read(file)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?;
    let path = format!("{name}.jpg");
    pictures.push((path.clone(), crate::previews::shrink(&data)?));
    Ok(path)
}

fn put(out: &Path, path: &str, write: impl FnOnce(&Path) -> std::io::Result<()>) -> Result<()> {
    check_path(path)?;
    let target = out.join(path);
    fs::create_dir_all(target.parent().unwrap_or(out))?;
    write(&target).map_err(|e| said("editor.cannot_write", "cannot write %1: %2", &[&path, &e]).into())
}

fn credits(doc: &Value, copied: &[String]) -> Vec<Value> {
    let list = doc["manifest"]["credits"].as_array().into_iter().flatten();
    let kept = list.filter_map(|c| {
        let named = |p: &str| {
            c["paths"].as_array().into_iter().flatten().any(|q| match q.as_str() {
                Some(q) if q.ends_with('/') => p.starts_with(q),
                Some(q) => p == q,
                None => false,
            })
        };
        let paths: Vec<&String> = copied.iter().filter(|p| named(p)).collect();
        (!paths.is_empty()).then(|| {
            let mut credit = c.clone();
            credit["paths"] = json!(paths);
            credit
        })
    });
    kept.collect()
}

pub fn make(dir: &Path, doc: &Value, picks: &[Pick], out: &Path, offer: &Offer) -> Result<Project> {
    let empty = !out.exists() || fs::read_dir(out)?.next().is_none();
    ensure!(empty, said("editor.not_empty_folder", "%1 is not an empty folder", &[&(out.display())]));
    ensure!(valid_id(&offer.id), said("editor.package_id", "%1 is not a package id", &[&offer.id]));
    let scene = &doc["scene"];
    let mut parts = fragment(scene, picks)?;
    let copied: Vec<String> = files(&parts).into_iter().collect();
    let (mut listed, mut pictures) = (Vec::new(), Vec::new());
    for (n, pick) in picks.iter().enumerate() {
        let mut part = json!({ "kind": pick.kind, "id": pick.id });
        if let Some(title) = offer.titles.get(&pick.key()).filter(|t| !t.trim().is_empty()) {
            part["title"] = json!(title.trim());
        }
        if let Some(file) = offer.thumbnails.get(&pick.key()) {
            part["thumbnail"] = json!(picture(file, &format!("parts/{n}"), &mut pictures)?);
        }
        listed.push(part);
    }
    parts["schema"] = scene["schema"].clone();
    parts["canvas"] = scene["canvas"].clone();
    parts["ext"] = json!({ "parts": listed });
    let mut manifest = json!({
        "schema": SCHEMA, "id": offer.id, "version": "1.0.0", "kind": "assets", "entry": "assets.json",
        "title": offer.title, "authors": [offer.attribution], "license": offer.license,
        "origin": { "kind": "original" }, "content": doc["manifest"].get("content").cloned()
            .unwrap_or(json!({ "rating": "general" })),
        "photosensitive": doc["manifest"]["photosensitive"].as_bool().unwrap_or(false),
        "requires": { "features": ["assets"] },
    });
    let kept = credits(doc, &copied);
    if !kept.is_empty() {
        manifest["credits"] = json!(kept);
    }
    if let Some(file) = &offer.thumbnail {
        manifest["previews"] = json!({ "thumbnail": picture(file, "preview", &mut pictures)? });
    }
    let mut project = Project::create(out, json!({ "manifest": manifest, "scene": parts }))?;
    for path in &copied {
        put(out, path, |target| fs::copy(dir.join(path), target).map(drop))?;
    }
    for (path, bytes) in &pictures {
        put(out, path, |target| fs::write(target, bytes))?;
    }
    project.settle()?;
    Ok(project)
}
