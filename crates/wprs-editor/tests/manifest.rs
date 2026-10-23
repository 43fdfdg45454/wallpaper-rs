use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, Rgba, RgbaImage};
use serde_json::{Value, json};
use std::fs;
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::{check, check_files};
use wprs_editor::inspect::fields;
use wprs_editor::meta;
use wprs_editor::ops::apply;
use wprs_editor::package::build;
use wprs_editor::starters::starter;
use wprs_editor::{Problem, Project};

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

fn action(value: Value) -> Action {
    serde_json::from_value(value).unwrap()
}

fn run(doc: &mut Value, value: Value) {
    let ops = plan(doc, &action(value)).unwrap();
    apply(doc, &ops).unwrap();
}

fn at(problems: &[Problem], pointer: &str) -> bool {
    problems.iter().any(|p| p.pointer == pointer && !p.warning)
}

#[test]
fn texts_are_translated_where_they_live() {
    let mut doc = starter("empty", "org.example.dusk", "Dusk", "en").unwrap();
    let title =
        |text: Value| json!({"translate": {"pointer": "/manifest/title", "language": "es", "text": text}});
    run(&mut doc, title(json!("Atardecer")));
    assert_eq!(doc["manifest"]["title"], json!({"en": "Dusk", "es": "Atardecer"}));
    let named =
        |languages: &[&str]| meta::title(&doc, &languages.iter().map(|l| l.to_string()).collect::<Vec<_>>());
    assert_eq!((named(&["es_ES.UTF-8"]), named(&[])), ("Atardecer".into(), "Dusk".into()));
    run(&mut doc, json!({"translate": {"pointer": "/manifest/title", "language": "", "text": "Evening"}}));
    run(&mut doc, title(Value::Null));
    assert_eq!(doc["manifest"]["title"], json!({"en": "Evening"}));
    let last = action(json!({"translate": {"pointer": "/manifest/title", "language": "en"}}));
    assert!(plan(&doc, &last).is_err(), "the title keeps a text");
    let description = |language: &str, text: &str| json!({"translate": {"pointer": "/manifest/description", "language": language, "text": text}});
    run(&mut doc, description("", "Hills at **dusk**."));
    assert_eq!(doc["manifest"]["description"], "Hills at **dusk**.");
    run(&mut doc, description("", ""));
    assert!(doc["manifest"].get("description").is_none());
    let changes = json!({"translate": {"pointer": "/manifest/changes", "language": "de", "text": "Neu"}});
    assert!(plan(&doc, &action(changes)).is_err(), "a text starts in the default language");
    doc["manifest"]["changes"] = json!({"de": "Neu"});
    assert!(at(&check(&doc), "/manifest/changes"), "a text needs the default language");
    run(&mut doc, json!({"remove": {"pointer": "/manifest/changes"}}));
    doc["manifest"]["strings"] = json!({"en": {"about": "Hills"}});
    doc["manifest"]["description"] = json!("@about");
    run(&mut doc, description("es", "Colinas"));
    assert_eq!(doc["manifest"]["strings"]["es"]["about"], "Colinas");
    let shown = fields(&doc, "/manifest");
    let rows = &shown.iter().find(|f| f.key == "description").unwrap().options;
    assert_eq!(
        rows[1],
        json!({"language": "es", "text": "Colinas", "pointer": "/manifest/strings/es/about"})
    );
    assert_eq!(check(&doc), []);
}

#[test]
fn adding_a_language_never_erases_a_text() {
    let mut doc = starter("empty", "org.example.mi", "Mi escena", "es").unwrap();
    assert_eq!(doc["manifest"]["default_locale"], "es");
    let add = |pointer: &str, language: &str| json!({"translate": {"pointer": pointer, "language": language, "add": true}});
    let put = |pointer: &str, language: &str, text: &str| json!({"translate": {"pointer": pointer, "language": language, "text": text}});
    run(&mut doc, add("/manifest/title", "es"));
    assert_eq!(doc["manifest"]["title"], "Mi escena");
    run(&mut doc, add("/manifest/title", "en"));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena", "en": "Mi escena"}));
    run(&mut doc, put("/manifest/title", "en", "My scene"));
    run(&mut doc, add("/manifest/title", "en"));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena", "en": "My scene"}));
    run(&mut doc, put("/manifest/description", "", "Colinas"));
    run(&mut doc, add("/manifest/description", "es"));
    assert_eq!(doc["manifest"]["description"], "Colinas");
    let shown = fields(&doc, "/manifest");
    let rows = &shown.iter().find(|f| f.key == "description").unwrap().options;
    assert_eq!((&rows[0]["language"], rows.len()), (&json!("es"), 1));
    run(&mut doc, put("/manifest/title", "en", ""));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena"}));
    assert!(plan(&doc, &action(put("/manifest/title", "es", " "))).is_err(), "the title keeps a text");
    assert!(plan(&doc, &action(add("/manifest/changes", "en"))).is_err(), "nothing to translate yet");
    for pointer in ["/manifest/version", "/manifest/inputs", "/manifest/requires/features", "/scene/canvas"] {
        assert!(plan(&doc, &action(put(pointer, "es", "x"))).is_err(), "{pointer} is not a text");
    }
    assert_eq!(check(&doc), []);
    let french = starter("particles", "org.example.fr", "Ma scène", "fr").unwrap();
    assert_eq!(french["manifest"]["default_locale"], "en", "the labels of the particles have no French");
    assert!(starter("empty", "org.example.x", "X", "not a language").is_err());
}

#[test]
fn the_inspector_shows_the_manifest_from_its_schema() {
    let mut doc = starter("parallax", "org.example.dawn", "Dawn", "en").unwrap();
    run(&mut doc, json!({"set": {"pointer": "/manifest/previews", "value": {}}}));
    run(&mut doc, json!({"translate": {"pointer": "/manifest/title", "language": "es", "text": "Alba"}}));
    let shown = fields(&doc, "/manifest");
    let top: Vec<&str> = shown.iter().filter(|f| f.depth == 0).map(|f| f.key.as_str()).collect();
    assert_eq!(top[..5], ["title", "description", "changes", "version", "id"]);
    assert!(!top.iter().any(|k| ["schema", "kind", "entry", "properties", "strings", "preview"].contains(k)));
    let field = |key: &str| shown.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{key}"));
    let widgets =
        ["title", "description", "authors", "inputs", "descriptors", "rating", "features", "thumbnail"]
            .map(|key| field(key).widget.as_str());
    assert_eq!(widgets, ["texts", "markup", "lines", "flags", "flags", "enum", "computed", "ref:thumbnail"]);
    assert_eq!((field("inputs").options.len(), &field("inputs").fixed), (5, &vec![json!("pointer")]));
    assert!(field("photosensitive").required && field("content").required && !field("links").required);
    assert_eq!(field("features").value, json!(["inputs.pointer"]));
    assert_eq!((field("kind").widget.as_str(), field("derived_from").widget.as_str()), ("enum", "list"));
    let locale = field("default_locale");
    assert_eq!((locale.widget.as_str(), locale.required), ("ref:language", true));
    assert!(locale.options.contains(&json!("en")) && locale.options.contains(&json!("es")));
}

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
