use super::*;

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        match entry.path().is_dir() {
            true => copy(&entry.path(), &to.join(entry.file_name())),
            false => drop(fs::copy(entry.path(), to.join(entry.file_name())).unwrap()),
        }
    }
}

#[test]
fn released_example_scenes_upgrade_into_valid_packages() {
    let released: BTreeMap<String, Value> =
        serde_json::from_str(include_str!("../fixture/released.json")).unwrap();
    let (tmp, root) =
        (tempfile::tempdir().unwrap(), Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes"));
    for (name, manifest) in released {
        let dir = tmp.path().join(&name);
        copy(&root.join(&name), &dir);
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        let mut scene: Value = serde_json::from_slice(&fs::read(dir.join("scene.json")).unwrap()).unwrap();
        let known = |layer: &Value| {
            layer["image"]["prop"].as_str().is_none_or(|p| manifest["properties"].get(p).is_some())
        };
        scene["layers"].as_array_mut().unwrap().retain(known);
        fs::write(dir.join("scene.json"), scene.to_string()).unwrap();
        let content = Content { rating: Rating::General, descriptors: Vec::new() };
        let (upgraded, renames) = upgrade(&dir, content, false, Origin::by(&[])).unwrap();
        let localized = manifest.to_string().contains(r#""es":"#);
        assert_eq!((upgraded.default_locale.is_some(), renames.len()), (localized, 0), "{name}");
        assert_eq!(upgraded.thumbnail().map(String::as_str), Some("preview.jpg"));
        build(&dir, tmp.path().join(format!("{name}.wpak")), &Limits::default()).unwrap();
    }
}

#[test]
fn upgrades_rename_hyphenated_properties_and_find_the_default_language() {
    let label = |es: &str, de: &str| json!({"es": es, "de": de});
    let mut manifest = json!({"schema": "wallpaper-rs.wpak/1", "id": "org.example.old", "version": "1.0.0",
        "title": "Old", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "properties": {
            "snow-speed": {"type": "slider", "min": 0, "max": 9, "step": 1, "default": 3, "label": label("Velocidad", "Tempo")},
            "snow_speed": {"type": "bool", "default": true, "visible_if": {"snow-speed": 3}, "label": label("Nieve", "Schnee")}},
        "presets": [{"id": "fast", "values": {"snow-speed": 9}}]});
    let mut scene = json!({"layers": [{"opacity": {"prop": "snow-speed", "scale": 0.1}}],
        "timelines": [{"id": "intro", "trigger": "prop:snow-speed"}]});
    let renames = upgrade_json(&mut manifest, Some(&mut scene)).unwrap();
    assert_eq!(renames, BTreeMap::from([("snow-speed".to_string(), "snow_speed_2".to_string())]));
    assert_eq!(manifest["properties"]["snow_speed"]["visible_if"], json!({"snow_speed_2": 3}));
    assert_eq!(manifest["presets"][0]["values"], json!({"snow_speed_2": 9}));
    assert_eq!(scene["layers"][0]["opacity"], json!({"prop": "snow_speed_2", "scale": 0.1}));
    assert_eq!(
        (&scene["timelines"][0]["trigger"], &manifest["default_locale"]),
        (&json!("prop:snow_speed_2"), &json!("de"))
    );
    let upgraded: Manifest = serde_json::from_value(manifest.clone()).unwrap();
    let missing: Vec<String> = upgraded.missing().iter().map(ToString::to_string).collect();
    assert_eq!(missing.len(), 3, "{missing:?}");
    assert!(missing[0].contains("content") && missing[2].contains("origin"));
    assert!(upgrade_json(&mut manifest, None).unwrap_err().to_string().contains("already"));
}

#[test]
fn renames_reach_conditions_of_both_spellings() {
    let mut manifest = json!({"properties": {"mood": {"type": "combo", "options": ["a", "b"], "default": "a"},
        "moody": {"type": "bool", "default": false},
        "tint": {"type": "color", "default": "#ffffff", "enabled_if": {"mood": "b"}, "visible_if": "mood == 'mood' || !moody"}}});
    rename_json(&mut manifest, None, &BTreeMap::from([("mood".to_string(), "feel".to_string())]));
    let tint = &manifest["properties"]["tint"];
    assert_eq!(tint["enabled_if"], json!({"feel": "b"}));
    assert_eq!(tint["visible_if"], "feel == 'mood' || !moody");
    assert!(manifest["properties"].get("feel").is_some() && manifest["properties"].get("mood").is_none());
}

#[test]
fn inputs_count_every_property_value_and_clicks() {
    let scene = |effects: &str, props: &str| {
        let scene = format!(
            r##"{{"schema": "wallpaper-rs.scene/1", "canvas": {{"size": [64, 32]}},
            "layers": [{{"id": "fill", "type": "solid", "color": "#336699", "effects": [{effects}]}}]}}"##
        );
        let properties = serde_json::from_str(props).unwrap();
        let manifest = Manifest { properties, ..v2(Kind::Scene) };
        let parsed = wprs_format::Scene::load(&scene, &manifest.properties).unwrap();
        (features::facts(&manifest, Some(&parsed), None), manifest)
    };
    let gated = r#"{"effect": "chromatic_aberration", "enabled": {"prop": "react"}, "params": {"amount": {"audio": "bass"}}}"#;
    let (facts, manifest) = scene(gated, r#"{"react": {"type": "bool", "default": false}}"#);
    assert_eq!(facts.inputs.iter().copied().collect::<Vec<_>>(), [Input::Audio]);
    let refused = features::check(&manifest, &facts).unwrap_err().to_string();
    assert!(refused.contains("audio at /layers/0/effects/0 of scene.json"), "{refused}");
    let ripple = r#"{"effect": "water_ripple", "params": {"click": {"prop": "touch"}}}"#;
    let (facts, _) =
        scene(ripple, r#"{"touch": {"type": "slider", "min": 0, "max": 8, "step": 1, "default": 0}}"#);
    assert_eq!(facts.inputs.iter().copied().collect::<Vec<_>>(), [Input::Pointer, Input::Clicks]);
    let (quiet, _) = scene(r#"{"effect": "water_ripple", "params": {"click": 0}}"#, "{}");
    assert_eq!(quiet.inputs.iter().copied().collect::<Vec<_>>(), [Input::Pointer]);
}
