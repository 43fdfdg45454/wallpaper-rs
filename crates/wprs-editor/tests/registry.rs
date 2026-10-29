#[path = "../../../tests/fixtures/features.rs"]
mod table;

use serde_json::Value;
use std::fs;
use wprs_editor::inspect::fields;
use wprs_editor::schema::{MANIFEST, SCENE, child, fits};
use wprs_editor::starters::starter;

const SKIPPED: [&str; 4] = ["type", "id", "schema", "layers"];
const BOUND: [&str; 7] = ["keys", "prop", "audio", "cursor", "clock", "script", "media"];

fn deep(root: &Value, schema: &Value, value: &Value, at: &str) {
    assert!(fits(root, schema, value), "{at} does not fit its schema: {value}");
    let children: Vec<(String, &Value)> = match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(list) => list.iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
        _ => Vec::new(),
    };
    for (key, item) in children {
        let next = child(root, schema, &key, value).unwrap_or_else(|| panic!("{at}/{key} has no schema"));
        deep(root, &next, item, &format!("{at}/{key}"));
    }
}

fn shown(doc: &Value, pointer: &str, missing: &mut Vec<String>) {
    match doc.pointer(pointer).cloned().unwrap_or_default() {
        Value::Object(map) if !BOUND.iter().any(|b| map.contains_key(*b)) => {
            let keys: Vec<String> = fields(doc, pointer).into_iter().map(|f| f.key).collect();
            let top = if pointer == "/manifest" { wprs_editor::meta::hidden(doc) } else { Vec::new() };
            let wanted = map.iter().flat_map(|(k, v)| match k.as_str() {
                "params" => v.as_object().into_iter().flatten().map(|(p, _)| p.clone()).collect(),
                _ => vec![k.clone()],
            });
            let map_like = keys.iter().all(|k| pointer.rsplit('/').next() == Some(k.as_str()));
            let skipped = |k: &String| map_like || SKIPPED.contains(&k.as_str()) || top.contains(&k.as_str());
            missing.extend(
                wanted.filter(|k| !skipped(k) && !keys.contains(k)).map(|k| format!("{pointer}/{k}")),
            );
            for (key, value) in map.iter().filter(|(k, _)| k.as_str() != "params") {
                if value.is_object() || value.is_array() {
                    shown(doc, &format!("{pointer}/{key}"), missing);
                }
            }
        }
        Value::Array(list) => (0..list.len()).for_each(|i| shown(doc, &format!("{pointer}/{i}"), missing)),
        _ => {}
    }
}

#[test]
fn every_feature_fixture_round_trips_through_the_schema_and_shows_in_the_inspector() {
    let dir = tempfile::tempdir().unwrap();
    let mut cases = table::cases();
    cases.push(("effects".into(), table::effects()));
    for (name, case) in &cases {
        table::build(name, case, dir.path()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let source = dir.path().join(name);
        let manifest: Value =
            serde_json::from_slice(&fs::read(source.join("manifest.json")).unwrap()).unwrap();
        deep(&MANIFEST, &MANIFEST, &manifest, name);
        let typed: wprs_format::Manifest = serde_json::from_value(manifest.clone()).unwrap();
        deep(&MANIFEST, &MANIFEST, &serde_json::to_value(&typed).unwrap(), name);
        let mut doc = starter("empty", "org.example.fixture", "Fixture", "en").unwrap();
        doc["manifest"] = manifest;
        if let Ok(text) = fs::read_to_string(source.join("scene.json")) {
            let scene: Value = serde_json::from_str(&text).unwrap();
            deep(&SCENE, &SCENE, &scene, name);
            let typed = wprs_scene::Scene::from_json(&text).unwrap();
            let written = serde_json::to_value(&typed).unwrap();
            deep(&SCENE, &SCENE, &written, name);
            assert_eq!(wprs_scene::Scene::from_json(&written.to_string()).unwrap(), typed, "{name}");
            doc["scene"] = scene;
        }
        let mut missing = Vec::new();
        shown(&doc, "/manifest", &mut missing);
        shown(&doc, "/scene", &mut missing);
        assert!(missing.is_empty(), "{name}: the inspector does not show {missing:?}");
    }
}
