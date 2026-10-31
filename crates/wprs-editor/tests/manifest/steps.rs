use super::*;

#[test]
fn inputs_and_features_follow_every_step() {
    state();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("sea");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.sea", "Sea", "en").unwrap()).unwrap();
    let act = |project: &mut Project, value: Value| project.act(&action(value));
    assert!(project.doc["manifest"].get("requires").is_none());
    act(&mut project, json!({"add_layer": {"kind": "spectrum"}})).unwrap();
    assert_eq!(project.doc["manifest"]["inputs"], json!(["audio"]));
    assert_eq!(project.doc["manifest"]["requires"], json!({"features": ["inputs.audio"]}));
    act(&mut project, json!({"set": {"pointer": "/manifest/inputs", "value": ["network"]}})).unwrap();
    assert_eq!(project.doc["manifest"]["inputs"], json!(["audio", "network"]));
    let typed = json!({"set": {"pointer": "/manifest/requires/features", "value": []}});
    assert!(act(&mut project, typed).is_err(), "features are never typed");
    act(&mut project, json!({"undo": {}})).unwrap();
    act(&mut project, json!({"undo": {}})).unwrap();
    assert!(
        project.doc["manifest"].get("requires").is_none() && project.doc["manifest"].get("inputs").is_none()
    );
    act(&mut project, json!({"redo": {}})).unwrap();
    project.save().unwrap();
    let mut manifest: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    manifest.as_object_mut().unwrap().retain(|k, _| k != "inputs" && k != "requires");
    fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    let mut project = Project::open(&dir).unwrap();
    assert!(at(&check(&project.doc), "/manifest/inputs"));
    build(&mut project, &root.path().join("sea.wpak"), None).unwrap();
    assert_eq!(project.history.labels().0, ["sync"]);
    let saved: Value = serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!((&saved["inputs"], project.dirty()), (&json!(["audio"]), false));
}

#[test]
fn inputs_the_editor_added_go_with_what_needed_them() {
    state();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("echo");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.echo", "Echo", "en").unwrap()).unwrap();
    let act = |project: &mut Project, value: Value| project.act(&action(value)).unwrap();
    let manifest = |project: &Project, key: &str| project.doc["manifest"].get(key).cloned();
    act(&mut project, json!({"set": {"pointer": "/manifest/requires", "value": {}}}));
    act(&mut project, json!({"set": {"pointer": "/manifest/requires/engine", "value": ">=0.1"}}));
    assert_eq!(manifest(&project, "requires"), Some(json!({"engine": ">=0.1"})));
    act(&mut project, json!({"add_layer": {"kind": "spectrum"}}));
    act(&mut project, json!({"set": {"pointer": "/manifest/inputs", "value": ["audio", "media"]}}));
    act(&mut project, json!({"remove_layer": {"id": "spectrum"}}));
    assert_eq!(manifest(&project, "inputs"), Some(json!(["media"])));
    assert_eq!(
        manifest(&project, "requires"),
        Some(json!({"engine": ">=0.1", "features": ["inputs.media"]}))
    );
    act(&mut project, json!({"undo": {}}));
    assert_eq!(manifest(&project, "inputs"), Some(json!(["audio", "media"])));
    let steps = project.history.done.len();
    assert!(
        act(&mut project, json!({"set": {"pointer": "/manifest/inputs", "value": ["media"]}})).is_empty()
    );
    assert_eq!(
        (project.history.done.len(), manifest(&project, "inputs")),
        (steps, Some(json!(["audio", "media"])))
    );
}

#[test]
fn first_version_manifests_upgrade_without_a_guessed_rating() {
    let mut doc = starter("particles", "org.example.old", "Old", "en").unwrap();
    doc["manifest"] = json!({"schema": wprs_format::SCHEMA_V1, "id": "org.example.old", "version": "1.0.0",
        "title": "Old", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png",
        "properties": {"snow-rate": {"type": "slider", "min": 0, "max": 400, "step": 10, "default": 160,
            "label": {"en": "Rate", "es": "Ritmo"}}}});
    doc["scene"]["particles"]["flakes"]["emitters"][0]["rate"] = json!({"prop": "snow-rate"});
    let top: Vec<String> =
        fields(&doc, "/manifest").into_iter().filter(|f| f.depth == 0).map(|f| f.key).collect();
    assert_eq!(top, ["title", "description", "version", "id", "authors", "license", "preview", "tags"]);
    let translated = json!({"translate": {"pointer": "/manifest/title", "language": "es", "text": "Viejo"}});
    assert!(plan(&doc, &action(translated)).is_err(), "first-version titles have one language");
    run(&mut doc, json!({"translate": {"pointer": "/manifest/title", "language": "", "text": "Older"}}));
    assert_eq!(check(&doc), []);
    let mut plain = doc.clone();
    run(&mut plain, json!({"upgrade": {}}));
    let problems = check(&plain);
    let missing = ["content", "photosensitive", "origin"].map(|f| at(&problems, &format!("/manifest/{f}")));
    assert_eq!(missing, [true; 3], "{problems:?}");
    run(&mut doc, json!({"upgrade": {"rating": "mature", "reasons": ["violence"], "photosensitive": false}}));
    assert_eq!(
        (&doc["manifest"]["schema"], doc["manifest"].get("preview")),
        (&json!(wprs_format::SCHEMA), None)
    );
    assert_eq!(doc["manifest"]["previews"], json!({"thumbnail": "preview.png"}));
    assert_eq!(
        (&doc["manifest"]["default_locale"], &doc["manifest"]["origin"]),
        (&json!("en"), &json!({"kind": "original"}))
    );
    assert_eq!(doc["manifest"]["content"], json!({"rating": "mature", "descriptors": ["violence"]}));
    assert!(doc["manifest"]["properties"]["snow_rate"].is_object());
    assert_eq!(doc["scene"]["particles"]["flakes"]["emitters"][0]["rate"], json!({"prop": "snow_rate"}));
    assert_eq!(check(&doc), []);
    assert!(plan(&doc, &action(json!({"upgrade": {}}))).unwrap_err().to_string().contains("already"));
}

#[test]
fn previews_are_copied_and_made_to_fit() {
    state();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("moon");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.moon", "Moon", "en").unwrap()).unwrap();
    let picture = |name: &str, w: u32, h: u32| {
        let path = root.path().join(name);
        RgbaImage::from_pixel(w, h, Rgba([20, 40, 80, 255])).save(&path).unwrap();
        path
    };
    let thumbnail = |file: &std::path::Path, animated: bool| {
        action(json!({"thumbnail": {"file": file, "animated": animated}}))
    };
    project.act(&thumbnail(&picture("big.png", 2048, 1024), false)).unwrap();
    assert_eq!(project.doc["manifest"]["previews"]["thumbnail"], "assets/preview.jpg");
    assert_eq!(image::image_dimensions(dir.join("assets/preview.jpg")).unwrap(), (1024, 512));
    let mut seed = 7u32;
    let noisy = |name: &str, side: u32, seed: &mut u32| {
        let path = root.path().join(name);
        let mut next = || {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (*seed >> 24) as u8
        };
        RgbaImage::from_fn(side, side, |_, _| Rgba([next(), next(), next(), 255])).save(&path).unwrap();
        assert!(fs::metadata(&path).unwrap().len() > 1 << 20);
        path
    };
    for (name, side) in [("noise.png", 1024), ("huge.png", 1400)] {
        project.act(&thumbnail(&noisy(name, side, &mut seed), false)).unwrap();
        let path = dir.join(project.doc["manifest"]["previews"]["thumbnail"].as_str().unwrap());
        assert!(fs::metadata(&path).unwrap().len() <= 1 << 20, "{name}");
        assert!(image::image_dimensions(&path).unwrap().0 <= 1024, "{name}");
    }
    project.act(&thumbnail(&picture("small.png", 64, 32), false)).unwrap();
    assert_eq!(project.doc["manifest"]["previews"]["thumbnail"], "assets/preview.png");
    assert!(project.act(&thumbnail(&root.path().join("small.png"), true)).is_err());
    let gif = root.path().join("loop.gif");
    let frames = (0..2).map(|i| {
        Frame::from_parts(
            RgbaImage::from_pixel(32, 16, Rgba([i * 200, 0, 0, 255])),
            0,
            0,
            Delay::from_numer_denom_ms(100, 1),
        )
    });
    GifEncoder::new(fs::File::create(&gif).unwrap()).encode_frames(frames).unwrap();
    project.act(&thumbnail(&gif, true)).unwrap();
    assert_eq!(project.doc["manifest"]["previews"]["animated"], "assets/preview-animated.gif");
    project.save().unwrap();
    build(&mut project, &root.path().join("moon.wpak"), None).unwrap();
    fs::copy(root.path().join("big.png"), dir.join("assets/preview.png")).unwrap();
    let problems = check_files(&dir, &project.doc);
    assert!(at(&problems, "/manifest/previews/thumbnail"), "{problems:?}");
    fs::copy(root.path().join("noise.png"), dir.join("assets/preview.png")).unwrap();
    let problems = check_files(&dir, &project.doc);
    let heavy = problems.iter().find(|p| p.message.contains("bytes")).expect("a preview over 1 MiB");
    assert_eq!(heavy.pointer, "/manifest/previews/thumbnail");
}
