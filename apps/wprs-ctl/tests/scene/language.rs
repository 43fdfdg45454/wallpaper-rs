use super::*;

pub fn show_in(dir: &Path, folder: &str, pointer: &str) -> Value {
    serde_json::from_str(&ok(dir, &["scene", "show", folder, pointer])).unwrap()
}

#[test]
fn new_scenes_name_the_language_of_their_title() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut spanish = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    spanish.args(["scene", "new", "mi", "--title", "Mi escena"]).current_dir(dir);
    spanish.env("XDG_STATE_HOME", dir.join("state")).env("LC_ALL", "es_ES.UTF-8").env_remove("LANGUAGE");
    assert!(spanish.status().unwrap().success());
    assert_eq!(show_in(dir, "mi", "/manifest/default_locale"), json!("es"));
    ok(dir, &["scene", "translate", "mi", "/manifest/title", "en", "My scene"]);
    let usage = |args: &[&str]| run(dir, &[&["scene", "translate", "mi", "/manifest/title"], args].concat());
    assert_eq!(
        (usage(&["de"]).status.code(), usage(&["de", "Hallo", "--remove"]).status.code()),
        (Some(2), Some(2))
    );
    ok(dir, &["scene", "translate", "mi", "/manifest/title", "de", "--add"]);
    ok(dir, &["scene", "translate", "mi", "/manifest/title", "es", "--add"]);
    let title = json!({"es": "Mi escena", "en": "My scene", "de": "Mi escena"});
    assert_eq!(show_in(dir, "mi", "/manifest/title"), title);
    ok(dir, &["scene", "translate", "mi", "/manifest/title", "de", ""]);
    ok(dir, &["scene", "remove", "mi", "/manifest/photosensitive"]);
    assert!(fails(dir, &["scene", "validate", "mi"]).trim_end().ends_with("Error: 1 problem"));
    ok(dir, &["scene", "new", "pt", "--from", "clock", "--title", "Relógio", "--locale", "pt-BR"]);
    assert_eq!(show_in(dir, "pt", "/manifest/default_locale"), json!("pt-BR"), "the title is in pt-BR");
    assert_eq!(show_in(dir, "pt", "/manifest/properties/color/label/pt-BR"), json!("Color"));
    assert!(ok(dir, &["scene", "validate", "pt"]).is_empty());
    let refused = run(dir, &["scene", "new", "bad", "--locale", "not a language"]);
    assert!(
        refused.status.code() == Some(2) && String::from_utf8_lossy(&refused.stderr).contains("such as en")
    );
}

#[test]
fn the_package_details_are_edited_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "dusk", "--from", "parallax", "--title", "Dusk"]);
    ok(dir, &["scene", "translate", "dusk", "/manifest/title", "es", "Atardecer"]);
    ok(dir, &["scene", "translate", "dusk", "/manifest/description", "", "Hills at **dusk**."]);
    ok(
        dir,
        &[
            "scene",
            "set",
            "dusk",
            "/manifest/content",
            r#"{"rating": "mature", "descriptors": ["violence"]}"#,
        ],
    );
    let credit = r#"[{"paths": ["scene.json"], "license": "CC0-1.0", "attribution": "Example"}]"#;
    ok(dir, &["scene", "set", "dusk", "/manifest/credits", credit]);
    ok(dir, &["scene", "add", "dusk", "/manifest/inputs/-", "network"]);
    assert!(fails(dir, &["scene", "set", "dusk", "/manifest/requires/features", "[]"]).contains("computed"));
    RgbaImage::from_pixel(1600, 900, Rgba([90, 60, 40, 255])).save(dir.join("shot.png")).unwrap();
    assert_eq!(ok(dir, &["scene", "thumbnail", "dusk", "shot.png"]), "assets/preview.jpg\n");
    assert!(
        ok(dir, &["scene", "open", "dusk"])
            .starts_with("Dusk (org.example.dusk 0.1.0): 5 layers, 0 problems")
    );
    ok(dir, &["scene", "build", "dusk", "-o", "dusk.wpak"]);
    let checked: Value = serde_json::from_str(&ok(dir, &["pack", "check", "dusk.wpak", "--json"])).unwrap();
    assert_eq!((&checked["rating"], &checked["inputs"]), (&json!("mature"), &json!(["pointer", "network"])));
    assert_eq!(checked["missing"], json!([]));
    ok(dir, &["scene", "open", "copy", "--from", "dusk.wpak"]);
    let copied = |pointer: &str| -> Value {
        serde_json::from_str(&ok(dir, &["scene", "show", "copy", pointer])).unwrap()
    };
    assert_eq!(copied("/manifest/title"), json!({"en": "Dusk", "es": "Atardecer"}));
    assert_eq!(
        (copied("/manifest/id"), copied("/manifest/previews")),
        (json!("org.example.dusk.copy"), json!({"thumbnail": "assets/preview.jpg"}))
    );
    ok(dir, &["scene", "translate", "dusk", "/manifest/title", "es", "--remove"]);
    assert_eq!(show_in(dir, "dusk", "/manifest/title"), json!({"en": "Dusk"}));
    assert!(fails(dir, &["scene", "upgrade", "dusk"]).contains("already"));
    let schema: Value = serde_json::from_str(&ok(dir, &["scene", "schema", "--manifest"])).unwrap();
    assert!(
        schema["$defs"]["Content"].is_object()
            && schema["properties"]["photosensitive"]["x-required"] == true
    );
}
