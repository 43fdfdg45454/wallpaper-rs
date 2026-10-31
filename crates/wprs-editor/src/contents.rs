use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use wprs_format::{MANIFEST, SIGNATURE};

pub fn used(doc: &Value) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = [MANIFEST.to_owned()].into();
    let m = &doc["manifest"];
    used.extend(crate::project::entry(doc).ok());
    used.extend(m["preview"].as_str().map(String::from));
    used.extend(
        m["previews"].as_object().into_iter().flatten().filter_map(|(_, p)| p.as_str().map(String::from)),
    );
    let credits = m["credits"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["paths"].as_array().into_iter().flatten());
    used.extend(credits.filter_map(Value::as_str).filter(|p| !p.ends_with('/')).map(String::from));
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    used.extend(assets.filter_map(|(_, a)| a["path"].as_str().map(String::from)));
    let scripts = doc.pointer("/scene/scripts").and_then(Value::as_array).into_iter().flatten();
    used.extend(scripts.filter_map(|s| s["path"].as_str().map(String::from)));
    let effects = doc.pointer("/scene/effects").and_then(Value::as_object).into_iter().flatten();
    let effects = effects.flat_map(|(_, e)| [&e["descriptor"], &e["wgsl"]]);
    used.extend(effects.filter_map(Value::as_str).map(String::from));
    used
}

pub fn files(dir: &Path) -> Vec<(String, u64)> {
    fn visit(root: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
        for item in fs::read_dir(root.join(prefix)).into_iter().flatten().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            match item.file_type() {
                Ok(t) if t.is_dir() => visit(root, &path, out),
                _ => out.push((path, item.metadata().map_or(0, |m| m.len()))),
            }
        }
    }
    let mut out = Vec::new();
    visit(dir, "", &mut out);
    out.sort();
    out
}

pub fn unused(dir: &Path, doc: &Value) -> Vec<String> {
    let (used, v2) = (used(doc), doc["manifest"]["schema"] == wprs_format::SCHEMA);
    let kept = |p: &String| used.contains(p) || p == SIGNATURE || (v2 && wprs_format::locale_file(p));
    files(dir).into_iter().map(|(path, _)| path).filter(|p| !kept(p)).collect()
}

pub fn prune(dir: &Path, doc: &Value) -> Result<Vec<String>> {
    let gone: Vec<String> = unused(dir, doc).into_iter().filter(|p| p.starts_with("assets/")).collect();
    gone.iter().try_for_each(|p| fs::remove_file(dir.join(p)))?;
    Ok(gone)
}
