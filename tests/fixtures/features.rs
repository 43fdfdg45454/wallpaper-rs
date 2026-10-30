use base64::Engine;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub fn cases() -> Vec<(String, Value)> {
    let table: Map<String, Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/features.json"
    )))
    .unwrap();
    table.into_iter().collect()
}

fn merge(base: &mut Value, more: &Value) {
    if let (Some(base), Some(more)) = (base.as_object_mut(), more.as_object()) {
        base.extend(more.clone());
    }
}

fn bytes(text: &str) -> Vec<u8> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    match (text.strip_prefix("fixture:"), text.strip_prefix("base64:")) {
        (Some(name), _) => fs::read(fixtures.join(name)).unwrap(),
        (_, Some(data)) => base64::engine::general_purpose::STANDARD.decode(data).unwrap(),
        _ => text.as_bytes().to_vec(),
    }
}

pub fn source(name: &str, case: &Value, dir: &Path) -> PathBuf {
    let source = dir.join(name);
    let (kind, entry) =
        (case["kind"].as_str().unwrap_or("scene"), case["entry"].as_str().unwrap_or("scene.json"));
    let id = format!("org.example.{}", name.replace(['.', '_'], "-"));
    let mut manifest = json!({ "schema": wprs_format::SCHEMA, "id": id, "version": "1.0.0", "title": name,
        "license": "CC0-1.0", "kind": kind, "entry": entry, "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "previews": { "thumbnail": "preview.png" } });
    merge(&mut manifest, &case["manifest"]);
    let mut scene = json!({ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } } });
    merge(&mut scene, &case["scene"]);
    let mut files = vec![("preview.png".to_owned(), bytes("fixture:green-64x32.png"))];
    files.push(("sky.png".to_owned(), bytes("fixture:green-64x32.png")));
    files.push(("manifest.json".to_owned(), manifest.to_string().into_bytes()));
    if matches!(kind, "scene" | "assets") {
        files.push((entry.to_owned(), scene.to_string().into_bytes()));
    }
    let given = case["files"].as_object().into_iter().flatten();
    files.extend(given.map(|(path, text)| (path.clone(), bytes(text.as_str().unwrap_or_default()))));
    for (path, data) in files {
        fs::create_dir_all(source.join(&path).parent().unwrap()).unwrap();
        fs::write(source.join(path), data).unwrap();
    }
    source
}

pub fn build(name: &str, case: &Value, dir: &Path) -> wprs_format::Result<PathBuf> {
    let out = dir.join(format!("{name}.wpak"));
    let source = source(name, case, dir);
    wprs_format::build(&source, &out, &wprs_format::Limits::default()).map(|_| out)
}

pub fn effects() -> Value {
    let used = newer_effects().map(|id| json!({ "effect": id }));
    let built = wprs_scene::builtin().iter().filter(|d| wprs_scene::newer_params(&d.id).next().is_some());
    let set = built.map(|d| {
        let params: Map<String, Value> = wprs_scene::newer_params(&d.id)
            .map(|p| (p.id.clone(), if p.kind.asset() { json!("sky") } else { json!(p.default) }))
            .collect();
        json!({ "effect": d.id, "params": params })
    });
    let layer = |(i, e): (usize, Value)| json!({ "id": format!("e{i}"), "type": "image", "image": "sky", "effects": [e] });
    let layers: Vec<Value> = used.chain(set).enumerate().map(layer).collect();
    json!({ "manifest": { "inputs": ["audio", "pointer", "clicks"] }, "scene": { "layers": layers } })
}

pub fn newer_effects() -> impl Iterator<Item = &'static str> {
    let first = |id: &str| wprs_format::features::BUILTIN.split_whitespace().any(|b| b == id);
    let newer = wprs_scene::builtin().iter().filter(move |d| !first(&d.id));
    wprs_scene::undrawn().iter().chain(newer).map(|d| d.id.as_str())
}
