mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Input, Kind, Limits, Manifest, Package, Platform, features, read};

fn glb(doc: &Value) -> Vec<u8> {
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = b"glTF".to_vec();
    for word in [2, 20 + text.len() as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(word.to_le_bytes());
    }
    out.extend(text);
    out
}

fn fox() -> Vec<u8> {
    glb(&json!({ "asset": { "version": "2.0" }, "nodes": [{ "name": "arm" }, { "name": "hand" }],
        "skins": [{ "joints": [0, 1] }], "accessors": [{ "count": 3 }],
        "meshes": [{ "name": "body", "primitives": [{ "attributes": { "POSITION": 0, "JOINTS_0": 0, "WEIGHTS_0": 0 } }] }] }))
}

fn manifest(features: &[&str], change: Value) -> Manifest {
    let mut value = serde_json::to_value(v2(Kind::Scene)).unwrap();
    value["requires"] = json!({ "features": features });
    value.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn load(
    manifest: &Manifest,
    scene: &Value,
    files: &[(&str, &[u8])],
) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    load_raw(&serde_json::to_value(manifest).unwrap(), scene, files)
}

fn load_raw(
    manifest: &Value,
    scene: &Value,
    files: &[(&str, &[u8])],
) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    let (json, text, png) = (manifest.to_string(), scene.to_string(), image(16, 16, ImageFormat::Png));
    let base: [(&str, &[u8]); 4] = [
        ("manifest.json", json.as_bytes()),
        ("preview.png", &png),
        ("sky.png", &png),
        ("scene.json", text.as_bytes()),
    ];
    read(Cursor::new(archive(&[&base[..], files].concat())), &Limits::default())
}

fn scene(layers: Value, more: Value) -> Value {
    let mut scene = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } }, "layers": layers });
    scene.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    scene
}

#[test]
fn meshes_are_read_from_the_package_and_refused_until_they_are_drawn() {
    let layers = json!([{ "id": "p", "type": "mesh", "mesh": "fox", "ik": [{ "chain": ["arm", "hand"], "target": [10, 10] }] }]);
    let assets = json!({ "assets": { "sky": { "path": "sky.png" }, "fox": { "path": "fox.glb" } } });
    let text = scene(layers, assets.clone());
    let package = load(&manifest(&["ik", "mesh"], json!({})), &text, &[("fox.glb", &fox())]).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), ["ik", "mesh"]);
    for platform in [Platform::Desktop, Platform::Android] {
        let refused =
            features::playable(&package.manifest, &package.facts, platform).unwrap_err().to_string();
        assert!(
            refused.contains(
                "ik (reaches for targets with chains of bones) and mesh (bends pictures with a skeleton)"
            ),
            "{refused}"
        );
    }
    let wrong = json!([{ "id": "p", "type": "mesh", "mesh": "fox", "ik": [{ "chain": ["arm", "tail"], "target": [1, 1] }] }]);
    let fault = load(&manifest(&["ik", "mesh"], json!({})), &scene(wrong, assets), &[("fox.glb", &fox())]);
    assert!(fault.map(drop).unwrap_err().to_string().contains("/layers/0/ik/0/chain/1 names \"tail\""));
    let broken = load(&manifest(&["ik", "mesh"], json!({})), &text, &[("fox.glb", b"glTF junk")]);
    assert!(
        broken
            .map(drop)
            .unwrap_err()
            .to_string()
            .contains("/assets/fox: fox.glb is not a binary glTF 2.0 file")
    );
}

#[test]
fn declared_script_parts_and_media_become_inputs() {
    let more = json!({ "scripts": [{ "path": "game.js", "uses": ["audio", "timers"] }],
        "timelines": [{ "id": "song", "duration": 1, "trigger": "media:track" }] });
    let text = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), more);
    let files: [(&str, &[u8]); 1] = [("game.js", b"export function update() {}")];
    let wanted = ["inputs.audio", "inputs.media", "media_info", "scripts", "scripts2", "timeline_events"];
    let fault =
        load(&manifest(&wanted, json!({ "inputs": ["audio"] })), &text, &files).map(drop).unwrap_err();
    assert!(
        fault.to_string().contains("the scene uses media at /timelines/0 of scene.json; add \"media\""),
        "{fault}"
    );
    let wanted = ["inputs.audio", "inputs.media", "media_info", "scripts", "scripts2"];
    let package = load(&manifest(&wanted, json!({ "inputs": ["audio", "media"] })), &text, &files).unwrap();
    assert_eq!(package.facts.inputs.iter().collect::<Vec<_>>(), [&Input::Audio, &Input::Media]);
    let declared = package.manifest.declared().unwrap();
    assert!(declared.audio && declared.media && !declared.cursor);
    let phone =
        features::playable(&package.manifest, &package.facts, Platform::Android).unwrap_err().to_string();
    assert!(phone.contains("media_info") && !phone.contains("scripts2"), "{phone}");
    assert_eq!(features::lacks("android", &package.facts.features), ["scripts", "scripts2"]);
}

#[test]
fn rgb_output_and_spanned_scenes_need_their_manifest_fields() {
    let lights =
        scene(json!([{ "id": "rgb", "type": "adjustment", "size": [32, 8], "rgb": true }]), json!({}));
    let fault =
        load(&manifest(&["capture", "rgb"], json!({})), &lights, &[]).map(drop).unwrap_err().to_string();
    assert!(
        fault.contains("permissions: the scene sends its colors to RGB lighting; add \"rgb\": true"),
        "{fault}"
    );
    let granted = json!({ "permissions": { "rgb": true } });
    let package = load(&manifest(&["capture", "rgb"], granted), &lights, &[]).unwrap();
    assert_eq!(features::lacks("android", &package.facts.features), ["rgb"]);
    assert!(features::playable(&package.manifest, &package.facts, Platform::Desktop).is_err());
    let networked = json!({ "permissions": { "network": { "allow": ["https://api.example.org"] } } });
    let fault =
        load(&manifest(&["capture", "rgb"], networked), &lights, &[]).map(drop).unwrap_err().to_string();
    assert!(fault.contains("permissions.network"), "{fault}");
    let plain = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), json!({}));
    let spread = json!({ "display": { "span": "all-outputs" } });
    let package = load(&manifest(&["display2", "spanned_scenes"], spread), &plain, &[]).unwrap();
    let desktop =
        features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err().to_string();
    assert!(desktop.contains("spanned_scenes (spreads its scene over every screen)"), "{desktop}");
    assert!(features::playable(&package.manifest, &package.facts, Platform::Android).is_ok());
    let flipped = json!({ "display": { "flip": "horizontal" } });
    let fault =
        load(&manifest(&["display2", "flip"], flipped), &plain, &[]).map(drop).unwrap_err().to_string();
    assert!(fault.contains("fit, background and flip apply to pictures and videos only"), "{fault}");
}

#[test]
fn missing_inputs_point_at_the_script_or_asset_that_needs_them() {
    let more = json!({ "scripts": [{ "path": "game.js", "uses": ["pointer"] }],
        "assets": { "sky": { "path": "sky.png" }, "cover": { "path": "sky.png", "media": "previous" } } });
    let text = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), more);
    let files: [(&str, &[u8]); 1] = [("game.js", b"export function update() {}")];
    let wanted = ["inputs.media", "inputs.pointer", "media_info", "scripts"];
    let pointer =
        load(&manifest(&wanted, json!({ "inputs": ["media"] })), &text, &files).map(drop).unwrap_err();
    assert!(pointer.to_string().contains("pointer at /scripts/0 of scene.json"), "{pointer}");
    let media =
        load(&manifest(&wanted, json!({ "inputs": ["pointer"] })), &text, &files).map(drop).unwrap_err();
    assert!(media.to_string().contains("media at /assets/cover of scene.json"), "{media}");
    load(&manifest(&wanted, json!({ "inputs": ["media", "pointer"] })), &text, &files).unwrap();
}

#[test]
fn released_scenes_keep_their_meaning() {
    let look = json!({ "properties": { "look": { "type": "combo", "options": ["media", "audio", "cursor"], "default": "media" } } });
    let mapped = json!({ "prop": "look", "map": { "media": "sky", "audio": "sky", "cursor": "sky" } });
    let layers = json!([{ "id": "sky", "type": "image", "image": mapped },
        { "id": "time", "type": "text", "text": { "clock": "HH:mm Sydney, Madrid" } }]);
    let package = load(&manifest(&["properties2"], look.clone()), &scene(layers, json!({})), &[]).unwrap();
    assert!(package.facts.inputs.is_empty() && !package.facts.features.contains("clock_dates"));
    let dated = json!([{ "id": "day", "type": "text", "text": { "date": "EEEE d MMMM" } }]);
    let fault = load(&manifest(&["properties2"], look), &scene(dated, json!({})), &[]).map(drop).unwrap_err();
    assert!(fault.to_string().contains("add \"clock_dates\""), "{fault}");
}

#[test]
fn newer_keys_with_their_default_values_still_name_a_feature() {
    let folder = json!({ "photos": { "type": "folder", "mode": "one", "max_files": 1000, "recursive": 0, "quality": {} } });
    let group = json!({ "id": "g", "type": "group", "space": "2d", "layers": [{ "id": "fx", "type": "adjustment", "redraw": false, "rgb": false, "depth_test": false }] });
    let more = json!({ "timelines": [{ "id": "t", "duration": 1, "wrap": false, "events": [] }] });
    let text = scene(json!([group]), more);
    let open = |features: &[&str]| {
        let mut value = serde_json::to_value(manifest(features, json!({}))).unwrap();
        value["properties"] = folder.clone();
        load_raw(&value, &text, &[]).map(drop)
    };
    let fault = open(&["folder", "properties2"]).unwrap_err().to_string();
    assert!(fault.contains("add \"newer_keys\""), "{fault}");
    open(&["folder", "newer_keys", "properties2"]).unwrap();
    let plain = scene(json!([{ "id": "fx", "type": "adjustment" }]), json!({}));
    assert!(!wprs_scene::newer_scene_keys(&plain));
}

#[test]
fn newer_manifest_keys_name_a_feature_whatever_their_value() {
    let base = serde_json::to_value(manifest(&["folder", "properties2"], json!({}))).unwrap();
    assert!(!wprs_scene::newer_manifest_keys(&base));
    let keys = [
        ("", "platforms", json!([])),
        ("", "permissions", json!({})),
        ("/requires", "packages", json!([])),
        ("/requires", "gpu", json!(null)),
        ("/display", "flip", json!(null)),
        ("/display", "span", json!(null)),
        ("/origin", "provider", json!(null)),
        ("/properties/photos", "mode", json!("one")),
        ("/properties/photos", "max_files", json!(1000)),
        ("/properties/photos", "recursive", json!(0)),
        ("/properties/photos", "quality", json!({})),
    ];
    let plain = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), json!({}));
    for (at, key, value) in keys {
        let mut raw = base.clone();
        raw["display"] = json!({});
        raw["origin"] = json!({ "kind": "original" });
        raw["properties"] = json!({ "photos": { "type": "folder" } });
        raw.pointer_mut(at).and_then(Value::as_object_mut).unwrap().insert(key.into(), value);
        assert!(wprs_scene::newer_manifest_keys(&raw), "{at}/{key}");
        let mut named = raw.clone();
        named["requires"]["features"] = json!(["folder", "newer_keys", "properties2"]);
        let read = |raw: &Value| load_raw(raw, &plain, &[]).map(drop);
        let fault = read(&raw).unwrap_err().to_string();
        assert!(fault.contains("\"newer_keys\""), "{at}/{key}: {fault}");
        read(&named).unwrap_or_else(|e| panic!("{at}/{key}: {e}"));
    }
}

#[test]
fn released_scenes_keep_their_own_locale_files() {
    let plain = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), json!({}));
    let files: [(&str, &[u8]); 2] =
        [("locale/data.json", br#"{"stars": 120}"#), ("locale/es.json", br#"{"hello": "x"}"#)];
    let package = load(&manifest(&[], json!({})), &plain, &files).unwrap();
    assert!(!package.facts.features.contains("locale_files") && package.manifest.string_files.is_empty());
    let declared = load(&manifest(&["locale_files"], json!({})), &plain, &files).map(drop).unwrap_err();
    assert!(declared.to_string().contains("locale/data.json"), "{declared}");
    let texts = json!({ "default_locale": "en", "title": "@title",
        "strings": { "en": { "title": "Dusk" }, "es": { "title": "Atardecer" } } });
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir(source.path().join("locale")).unwrap();
    let png = image(16, 16, ImageFormat::Png);
    let text = (manifest(&[], texts).to_json().unwrap(), plain.to_string());
    let written: [(&str, &[u8]); 7] = [
        ("manifest.json", text.0.as_bytes()),
        ("scene.json", text.1.as_bytes()),
        ("preview.png", &png),
        ("sky.png", &png),
        ("locale/data.json", br#"{"stars": 120}"#),
        ("locale/fr.json", br#"{"Good morning": "Bonjour"}"#),
        ("locale/es.json", br#"{"hello": "hola"}"#),
    ];
    written.iter().for_each(|(name, data)| std::fs::write(source.path().join(name), data).unwrap());
    let (manifest, facts) = wprs_format::inspect(source.path()).unwrap();
    features::check(&manifest, &facts).unwrap();
    assert!(manifest.string_files.is_empty() && !facts.features.contains("locale_files"));
    let noted: Vec<&str> = facts.notes.iter().filter_map(|n| n.split(' ').next()).collect();
    assert_eq!(noted, ["locale/es.json", "locale/fr.json"]);
    assert!(wprs_format::fill(source.path()).unwrap().is_empty());
    let out = source.path().join("dusk.wpak");
    wprs_format::build(source.path(), &out, &Limits::default()).unwrap();
    let package = wprs_format::open(&out, &Limits::default()).unwrap();
    assert!(package.manifest.features().is_empty() && package.manifest.string_files.is_empty());
    assert_eq!(package.manifest.title_in(&["fr".into()]), "Dusk");
}
