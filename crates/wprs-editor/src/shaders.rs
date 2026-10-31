use crate::ops::{Op, escape};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Descriptor, MAX_EFFECT_FILES, MAX_WGSL};

fn text(path: &Path) -> Result<String> {
    let size = fs::metadata(path)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&path.display(), &e]))?
        .len();
    ensure!(
        size <= MAX_WGSL as u64,
        said("editor.larger_than_kib", "%1 is larger than %2 KiB", &[&(path.display()), &(MAX_WGSL >> 10)])
    );
    fs::read_to_string(path)
        .map_err(|e| said("editor.not_text", "%1 is not a text file: %2", &[&path.display(), &e]).into())
}

pub fn import(dir: &Path, doc: &Value, descriptor: &Path, wgsl: Option<&Path>) -> Result<Vec<Op>> {
    let shader = wgsl.map_or_else(|| descriptor.with_extension("wgsl"), Path::to_path_buf);
    let (json, code) = (text(descriptor)?, text(&shader)?);
    let parsed: Value = serde_json::from_str(&json)
        .map_err(|e| said("editor.descriptor_json", "the descriptor is not valid JSON: %1", &[&e]))?;
    let id = parsed["id"]
        .as_str()
        .ok_or_else(|| said("editor.descriptor_id", "the descriptor has no id", &[]))?
        .to_owned();
    Descriptor::packaged(&id, &json, &code)
        .map_err(|e| anyhow!(said("editor.effect", "the effect %1 %2", &[&id, &e])))?;
    let effects = doc.pointer("/scene/effects").and_then(Value::as_object);
    let known = effects.is_some_and(|e| e.contains_key(&id));
    let count = effects.map_or(0, Map::len);
    ensure!(
        known || count < MAX_EFFECT_FILES,
        said("editor.scene_most_effects", "a scene has at most %1 effects of its own", &[&MAX_EFFECT_FILES])
    );
    fs::create_dir_all(dir.join("effects"))?;
    let paths = [format!("effects/{id}.json"), format!("effects/{id}.wgsl")];
    for (path, body) in paths.iter().zip([&json, &code]) {
        fs::write(dir.join(path), body)
            .map_err(|e| said("editor.cannot_write", "cannot write %1: %2", &[&path, &e]))?;
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
