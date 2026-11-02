use crate::shelf::{Pick, files, fragment};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use wprs_core::message::said;
use wprs_format::{Kind, Limits, Manifest, Package, Part, open, parts};

pub(crate) type Opened = Package<BufReader<File>>;

pub(crate) fn opened(file: &Path) -> Result<(Opened, Value, Vec<Part>)> {
    let mut package = open(file, &Limits::default())?;
    ensure!(
        package.manifest.kind == Kind::Assets,
        said("editor.not_assets_package", "%1 is not an assets package", &[&(file.display())])
    );
    let entry = package.manifest.entry.clone();
    let doc: Value = serde_json::from_slice(&package.read_file(&entry)?)?;
    let offered = parts(&package.scene()?)?;
    Ok((package, doc, offered))
}

pub fn credit(manifest: &Manifest, path: &str) -> (String, String, Option<String>) {
    let named = |q: &String| if q.ends_with('/') { path.starts_with(q.as_str()) } else { q == path };
    let found = manifest.credits.iter().find(|c| c.paths.iter().any(named));
    let cut = |t: String| t.chars().take(256).collect::<String>();
    match found {
        Some(c) => (c.license.clone(), c.attribution.clone(), c.url.clone()),
        None => (manifest.license.clone(), cut(manifest.authors.join(", ")), None),
    }
}

pub fn browse(file: &Path, cache: Option<&Path>) -> Result<Value> {
    let (mut package, doc, offered) = opened(file)?;
    let m = package.manifest.clone();
    let mut listed = Vec::new();
    for part in offered {
        let pick = Pick { kind: part.kind.clone(), id: part.id.clone() };
        let used = files(&fragment(&doc, std::slice::from_ref(&pick))?);
        let files: Vec<Value> = used
            .iter()
            .map(|p| {
                let (license, attribution, url) = credit(&m, p);
                json!({ "path": p, "license": license, "attribution": attribution, "url": url })
            })
            .collect();
        let thumbnail = match (cache, &part.thumbnail) {
            (Some(cache), Some(path)) => {
                let target = cache.join(format!("{}-{}", m.id, m.version)).join(path);
                if !target.exists() {
                    fs::create_dir_all(target.parent().unwrap_or(cache))?;
                    package.copy_file(path, &mut File::create(&target)?)?;
                }
                Some(target.to_string_lossy().into_owned())
            }
            _ => None,
        };
        let title = part.title.clone().unwrap_or_else(|| part.id.clone());
        listed.push(json!({ "key": pick.key(), "kind": part.kind, "id": part.id, "title": title,
            "thumbnail": thumbnail, "files": files }));
    }
    Ok(json!({ "id": m.id, "version": m.version, "title": m.title_in(&wprs_core::locale::languages()),
        "license": m.license, "authors": m.authors, "parts": listed }))
}
