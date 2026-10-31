use super::*;

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
