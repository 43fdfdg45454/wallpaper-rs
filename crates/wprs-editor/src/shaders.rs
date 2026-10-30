use crate::ops::{Op, escape};
use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;
use wprs_scene::{Descriptor, MAX_EFFECT_FILES, MAX_WGSL};

fn text(path: &Path) -> Result<String> {
    let size = fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?.len();
    ensure!(size <= MAX_WGSL as u64, "{} is larger than {} KiB", path.display(), MAX_WGSL >> 10);
    fs::read_to_string(path).with_context(|| format!("{} is not a text file", path.display()))
}

pub fn import(dir: &Path, doc: &Value, descriptor: &Path, wgsl: Option<&Path>) -> Result<Vec<Op>> {
    let shader = wgsl.map_or_else(|| descriptor.with_extension("wgsl"), Path::to_path_buf);
    let (json, code) = (text(descriptor)?, text(&shader)?);
    let parsed: Value = serde_json::from_str(&json).context("the descriptor is not valid JSON")?;
    let id = parsed["id"].as_str().context("the descriptor has no id")?.to_owned();
    Descriptor::packaged(&id, &json, &code).map_err(|e| anyhow!("the effect {id} {e}"))?;
    let effects = doc.pointer("/scene/effects").and_then(Value::as_object);
    let known = effects.is_some_and(|e| e.contains_key(&id));
    let count = effects.map_or(0, Map::len);
    ensure!(known || count < MAX_EFFECT_FILES, "a scene has at most {MAX_EFFECT_FILES} effects of its own");
    fs::create_dir_all(dir.join("effects"))?;
    let paths = [format!("effects/{id}.json"), format!("effects/{id}.wgsl")];
    for (path, body) in paths.iter().zip([&json, &code]) {
        fs::write(dir.join(path), body).with_context(|| format!("cannot write {path}"))?;
    }
    let value = json!({ "descriptor": paths[0], "wgsl": paths[1] });
    let path = format!("/scene/effects/{}", escape(&id));
    Ok(match (effects.is_some(), known) {
        (false, _) => vec![Op::Add {
            path: "/scene/effects".into(),
            value: Value::Object([(id, value)].into_iter().collect()),
        }],
        (true, true) => vec![Op::Replace { path, value }],
        (true, false) => vec![Op::Add { path, value }],
    })
}
