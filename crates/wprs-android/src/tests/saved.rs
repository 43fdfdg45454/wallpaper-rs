use super::{ask, renderer};
use crate::{library, store};
use serde_json::{Value, json};
use std::fs::{self, File};

pub(super) fn saved_settings() {
    let source = tempfile::tempdir().unwrap();
    let layers = json!([{ "id": "sky", "type": "solid", "color": "#336699" }]);
    let text = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers,
        "timelines": [{ "id": "glow", "duration": 1, "autoplay": false, "trigger": "prop:again" }] });
    fs::write(source.path().join("scene.json"), text.to_string()).unwrap();
    let properties = json!({ "mood": { "type": "combo", "default": 1, "options": [1, 2] },
        "again": { "type": "button", "enabled_if": "mood == 2" }, "key": { "type": "text", "secret": true },
        "tools": { "type": "group", "enabled_if": "mood == 2" }, "later": { "type": "button", "group": "tools" },
        "photos": { "type": "folder" } });
    let manifest = json!({ "schema": wprs_format::SCHEMA, "id": "org.example.knobs", "version": "1.2.0", "title": "Knobs",
        "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "content": { "rating": "general" },
        "photosensitive": false, "origin": { "kind": "original" }, "properties": properties,
        "requires": { "features": ["folder", "properties2"] } });
    fs::write(source.path().join("manifest.json"), manifest.to_string()).unwrap();
    let built = source.path().join("knobs.wpak");
    wprs_format::build(source.path(), &built, &wprs_format::Limits::default()).unwrap();
    let uid = store::store(&mut File::open(&built).unwrap(), "knobs.wpak").unwrap().0;
    if renderer().is_some() {
        let color = library::with(|lib| Ok(lib.item(&uid)?.meta.color)).unwrap();
        assert_eq!(color, Some(0x336699), "a scene without a preview is drawn for its thumbnail");
        assert!(store::thumb_file(&uid, "jpg").unwrap().is_file());
    }
    let form = |key: &str| ask(json!({ "op": "scene", "uid": uid, "languages": [] }))[key].clone();
    assert!(ask(json!({ "op": "press", "uid": uid, "key": "again" }))["error"].is_string());
    assert!(ask(json!({ "op": "press", "uid": uid, "key": "later" }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "item", "uid": uid }))["lacks"], json!(["folder"]));
    assert_eq!(ask(json!({ "op": "property", "uid": uid, "key": "mood", "value": 2 })), Value::Null);
    assert_eq!(ask(json!({ "op": "press", "uid": uid, "key": "again" })), Value::Null);
    assert_eq!(crate::saved::pressed("org.example.knobs"), ["again"]);
    assert!(
        ask(json!({ "op": "property", "uid": uid, "key": "key", "value": "hunter2" }))["error"].is_string()
    );
    assert_eq!(ask(json!({ "op": "secret", "uid": uid, "key": "key", "text": "hunter2" })), Value::Null);
    let secret = form("fields").as_array().unwrap().iter().find(|f| f["key"] == "key").unwrap().clone();
    assert_eq!((&secret["filled"], &secret["value"]), (&json!(true), &Value::Null));
    assert!(
        !fs::read_to_string(store::dir().unwrap().join("settings.toml"))
            .unwrap_or_default()
            .contains("hunter2")
    );
    assert_eq!(ask(json!({ "op": "save", "uid": uid, "name": "Storm" })), Value::Null);
    let saved = form("saved");
    let preset = saved[0]["value"].as_str().unwrap().to_owned();
    assert_eq!(saved[0]["label"], json!("Storm"));
    assert_eq!(ask(json!({ "op": "rename", "preset": preset, "name": "Gale" })), Value::Null);
    let exported = ask(json!({ "op": "export", "uid": uid, "preset": "gale" }));
    let document: Value = serde_json::from_str(exported.as_str().unwrap()).unwrap();
    assert_eq!((&document["version"], &document["values"]), (&json!("1.2.0"), &json!({ "mood": 2 })));
    assert_eq!(ask(json!({ "op": "reset", "uid": uid })), Value::Null);
    assert_eq!(ask(json!({ "op": "preset", "uid": uid, "preset": "Gale" })), Value::Null);
    assert_eq!(
        form("fields").as_array().unwrap().iter().find(|f| f["key"] == "mood").unwrap()["value"],
        json!(2)
    );
    let warned = ask(
        json!({ "op": "import", "uid": uid, "text": exported.as_str().unwrap().replace("\"mood\"", "\"gone\""), "name": "" }),
    );
    assert_eq!(warned, json!(["gone is not a setting of org.example.knobs"]));
    let foreign = exported.as_str().unwrap().replace("org.example.knobs", "org.example.other");
    assert!(ask(json!({ "op": "import", "uid": uid, "text": foreign, "name": "" }))["error"].is_string());
    assert_eq!(ask(json!({ "op": "import", "uid": uid, "text": exported, "name": "Copy" })), json!([]));
    assert_eq!(form("saved").as_array().map(Vec::len), Some(2));
    assert_eq!(ask(json!({ "op": "forget", "preset": preset })), Value::Null);
    assert_eq!(form("saved")[0]["label"], json!("Copy"));
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
