use clap::CommandFactory;
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use wprs_ctl::Cli;
use wprs_editor::{ACTIONS, Action};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null()).env("XDG_STATE_HOME", dir.join("state"));
    command.env("LC_ALL", "C");
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    command.output().unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn fails(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(1), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8(out.stderr).unwrap()
}

fn show(dir: &Path, pointer: &str) -> Value {
    serde_json::from_str(&ok(dir, &["scene", "show", "snow", pointer])).unwrap()
}

#[test]
fn a_scene_is_made_edited_and_built_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "particles", "--title", "Snow"]);
    assert_eq!(show(dir, "/manifest/id"), "org.example.snow");
    assert!(
        ok(dir, &["scene", "open", "snow"])
            .starts_with("Snow (org.example.snow 0.1.0): 2 layers, 0 problems")
    );
    RgbaImage::from_pixel(8, 8, Rgba([200, 220, 255, 255])).save(dir.join("moon.png")).unwrap();
    assert_eq!(ok(dir, &["scene", "import", "snow", "moon.png"]), "moon\n");
    assert_eq!(ok(dir, &["scene", "add-layer", "snow", "image", "--source", "moon", "--at", "1"]), "image\n");
    ok(dir, &["scene", "set", "snow", "/scene/layers/1/transform/position", "[1500, 200]"]);
    ok(dir, &["scene", "set", "snow", "/scene/canvas/background", "#102030"]);
    ok(dir, &["scene", "add", "snow", "/scene/timelines", r#"[{"id": "rise", "duration": 4}]"#]);
    ok(dir, &["scene", "keyframe", "snow", "/scene/layers/1/opacity", "0", "0", "--timeline", "rise"]);
    ok(dir, &["scene", "keyframe", "snow", "/scene/layers/1/opacity", "4", "1", "--ease", "out_cubic"]);
    assert_eq!(
        show(dir, "/scene/layers/1/opacity"),
        json!({"keys": [[0.0, 0], [4.0, 1, "out_cubic"]], "timeline": "rise"})
    );
    assert_eq!(ok(dir, &["scene", "duplicate-layer", "snow", "image"]), "image_copy\n");
    ok(dir, &["scene", "group", "snow", "image", "image_copy", "--id", "moons"]);
    ok(dir, &["scene", "move-layer", "snow", "moons", "0"]);
    ok(dir, &["scene", "hide", "snow", "sky"]);
    ok(dir, &["scene", "lock", "snow", "flakes"]);
    ok(dir, &["scene", "solo", "snow", "flakes"]);
    ok(dir, &["scene", "ungroup", "snow", "moons"]);
    ok(dir, &["scene", "remove-layer", "snow", "image_copy"]);
    ok(dir, &["scene", "move", "snow", "/scene/layers/0", "/scene/layers/2"]);
    ok(dir, &["scene", "remove", "snow", "/scene/ext/editor/solo"]);
    let ids: Vec<Value> =
        show(dir, "/scene/layers").as_array().unwrap().iter().map(|l| l["id"].clone()).collect();
    assert_eq!(ids, [json!("sky"), json!("flakes"), json!("image")]);
    let history = ok(dir, &["scene", "history", "snow"]);
    assert_eq!(history.lines().count(), 17, "{history}");
    ok(dir, &["scene", "undo", "snow"]);
    assert_eq!(show(dir, "/scene/ext/editor/solo"), "flakes");
    ok(dir, &["scene", "redo", "snow"]);
    assert!(fails(dir, &["scene", "redo", "snow"]).contains("nothing to redo"));
    assert!(ok(dir, &["scene", "history", "snow"]).lines().all(|l| !l.ends_with("(undone)")));
    let fields: Value =
        serde_json::from_str(&ok(dir, &["scene", "fields", "snow", "/scene/layers/2"])).unwrap();
    assert!(
        fields.as_array().unwrap().iter().any(|f| f["key"] == "image" && f["options"] == json!(["moon"]))
    );
    ok(dir, &["scene", "set", "snow", "/scene/layers/2/opacity", "3"]);
    let problems = fails(dir, &["scene", "validate", "snow"]);
    assert!(problems.trim_end().ends_with("1 problem") && ok_lines(dir).contains("/scene/layers/2/opacity"));
    assert!(fails(dir, &["scene", "build", "snow"]).contains("/scene/layers/2/opacity"));
    ok(dir, &["scene", "undo", "snow"]);
    let built: Value =
        serde_json::from_str(&ok(dir, &["scene", "build", "snow", "-o", "snow.wpak", "--json"])).unwrap();
    assert_eq!(built["path"], "snow.wpak");
    assert!(ok(dir, &["pack", "validate", "snow.wpak"]).starts_with("ok scene org.example.snow 0.1.0"));
    ok(dir, &["scene", "open", "copy", "--from", "snow.wpak"]);
    assert!(dir.join("copy/assets/moon.png").is_file());
    ok(dir, &["scene", "set", "copy", "/scene/canvas/background", "#111111"]);
    ok(dir, &["scene", "set", "copy", "/scene/canvas/background", "#222222"]);
    assert_eq!(ok(dir, &["scene", "history", "copy"]), "set\nset\n");
    ok(dir, &["scene", "undo", "copy"]);
    assert_eq!(ok(dir, &["scene", "show", "copy", "/scene/canvas/background"]).trim(), "\"#111111\"");
    assert_eq!(ok(dir, &["scene", "add-layer", "copy", "text", "--name", "Título"]), "titulo\n");
    assert_eq!(ok(dir, &["scene", "show", "copy", "/scene/layers/3/text"]).trim(), "\"Título\"");
    std::fs::write(dir.join("snow/assets/stale.png"), b"x").unwrap();
    assert_eq!(ok(dir, &["scene", "prune", "snow"]), "assets/stale.png\n");
    let schema: Value = serde_json::from_str(&ok(dir, &["scene", "schema"])).unwrap();
    assert!(schema["$defs"]["Layer"].is_object());
    assert!(run(dir, &["scene", "preview", "snow", "-o", "DP-1"]).status.code() == Some(1));
}

fn ok_lines(dir: &Path) -> String {
    String::from_utf8(run(dir, &["scene", "validate", "snow"]).stdout).unwrap()
}

fn fields(value: Value) -> Vec<String> {
    let refused = serde_json::from_value::<Action>(value).unwrap_err().to_string();
    refused.split('`').skip(3).step_by(2).map(String::from).collect()
}

#[test]
fn every_scene_editor_action_has_a_scene_command_with_the_same_arguments() {
    assert_eq!(fields(json!({"": {}})), ACTIONS);
    let scene = Cli::command().find_subcommand("scene").cloned().unwrap();
    for name in ACTIONS {
        let command = scene.find_subcommand(name.replace('_', "-")).unwrap_or_else(|| panic!("scene {name}"));
        let ids: Vec<&str> = command.get_arguments().map(|a| a.get_id().as_str()).collect();
        for field in fields(json!({name: {"": null}})) {
            assert!(ids.contains(&field.as_str()), "scene {name} lacks {field}");
        }
        assert!(ids.contains(&"dir"), "scene {name} takes the folder");
    }
}

#[test]
fn an_edited_copy_of_a_signed_scene_carries_no_signature_of_the_original() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let author = ok(dir, &["repo", "keygen", "author", "--no-password"]).trim().to_owned();
    ok(dir, &["scene", "new", "orig"]);
    ok(dir, &["scene", "set", "orig", "/manifest/authors", r#"["Ana"]"#]);
    ok(dir, &["scene", "build", "orig", "--key", "author.key", "-o", "orig.wpak"]);
    assert!(ok(dir, &["pack", "validate", "orig.wpak", "--key", "author.pub"]).contains(&author));
    assert!(ok(dir, &["scene", "validate", "orig"]).contains("manifest.json.minisig"));
    ok(dir, &["scene", "open", "copy", "--from", "orig.wpak"]);
    assert!(!dir.join("copy/manifest.json.minisig").exists());
    assert_eq!(ok(dir, &["scene", "show", "copy", "/manifest/id"]).trim(), "\"org.example.orig.copy\"");
    let ancestor = json!({"author": "Ana", "title": "orig", "license": "CC0-1.0"});
    assert_eq!(
        show_in(dir, "copy", "/manifest/origin"),
        json!({"kind": "derived", "derived_from": [ancestor]})
    );
    assert_eq!(run(dir, &["scene", "show", "copy", "/manifest/authors"]).status.code(), Some(1));
    ok(dir, &["scene", "set", "copy", "/scene/canvas/background", "#ff0000"]);
    let built: Value = serde_json::from_str(&ok(dir, &["scene", "build", "copy", "--json"])).unwrap();
    assert_eq!(built["path"], json!(dir.canonicalize().unwrap().join("org.example.orig.copy-0.1.0.wpak")));
    let checked = run(dir, &["pack", "validate", "org.example.orig.copy-0.1.0.wpak", "--key", "author.pub"]);
    assert!(!String::from_utf8_lossy(&checked.stdout).contains(&author));
    ok(dir, &["scene", "build", "orig", "-o", "plain.wpak"]);
    assert!(
        !dir.join("orig/manifest.json.minisig").exists(),
        "a build without a key drops the old signature"
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

fn show_in(dir: &Path, folder: &str, pointer: &str) -> Value {
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
    assert_eq!(show_in(dir, "pt", "/manifest/default_locale"), json!("en"), "the clock labels have no pt-BR");
    let refused = run(dir, &["scene", "new", "bad", "--locale", "not a language"]);
    assert!(
        refused.status.code() == Some(2) && String::from_utf8_lossy(&refused.stderr).contains("such as en")
    );
}

#[test]
fn scene_upgrade_takes_what_pack_upgrade_takes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for name in ["old", "older"] {
        ok(dir, &["scene", "new", name, "--from", "particles"]);
        ok(dir, &["scene", "set", name, "/manifest/schema", "wallpaper-rs.wpak/1"]);
        for field in ["content", "photosensitive", "origin", "default_locale"] {
            ok(dir, &["scene", "remove", name, &format!("/manifest/{field}")]);
        }
    }
    ok(dir, &["scene", "validate", "old"]);
    ok(dir, &["scene", "upgrade", "old", "--rating", "general", "--photosensitive", "false"]);
    ok(dir, &["scene", "validate", "old"]);
    assert_eq!(
        (show_in(dir, "old", "/manifest/default_locale"), show_in(dir, "old", "/manifest/origin")),
        (json!("en"), json!({"kind": "original"}))
    );
    ok(dir, &["scene", "upgrade", "older"]);
    let listed = String::from_utf8(run(dir, &["scene", "validate", "older"]).stdout).unwrap();
    for field in ["content", "photosensitive", "origin"] {
        assert!(listed.contains(&format!("/manifest/{field}: {field}: is required")), "{listed}");
    }
    ok(dir, &["scene", "undo", "older"]);
    assert_eq!(show_in(dir, "older", "/manifest/schema"), json!("wallpaper-rs.wpak/1"));
}

#[test]
fn user_properties_and_presets_are_edited_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "particles"]);
    assert_eq!(
        ok(dir, &["scene", "add-property", "snow", "group", "--id", "look", "--label", "Look"]),
        "look\n"
    );
    assert_eq!(ok(dir, &["scene", "add-property", "snow", "combo", "--group", "look"]), "combo_1\n");
    ok(dir, &["scene", "rename-property", "snow", "combo_1", "mood"]);
    assert_eq!(ok(dir, &["scene", "add-property", "snow", "button"]), "button_1\n");
    assert!(fails(dir, &["scene", "condition", "snow", "button_1", "mood =="]).contains("ends too early"));
    ok(dir, &["scene", "condition", "snow", "button_1", "mood == 'option_2'", "--enabled"]);
    ok(dir, &["scene", "move-property", "snow", "button_1", "0", "--group", "look"]);
    assert_eq!(
        show(dir, "/manifest/properties/button_1"),
        json!({"type": "button", "group": "look", "order": 1, "enabled_if": "mood == 'option_2'"})
    );
    assert_eq!(show(dir, "/manifest/properties/mood/order"), 2);
    assert_eq!(ok(dir, &["scene", "add-preset", "snow", "--label", "Two"]), "preset_1\n");
    ok(dir, &["scene", "set", "snow", "/manifest/presets/0/values/mood", "option_2"]);
    assert!(show(dir, "/manifest/requires/features").as_array().unwrap().contains(&json!("properties2")));
    let fields: Value =
        serde_json::from_str(&ok(dir, &["scene", "fields", "snow", "/manifest/presets/0"])).unwrap();
    assert!(fields.as_array().unwrap().iter().any(|f| f["key"] == "mood" && f["set"] == true));
    assert_eq!(ok_lines(dir), "");
    std::fs::create_dir_all(dir.join("snow/scripts")).unwrap();
    std::fs::write(dir.join("snow/scripts/look.js"), "export const update = () => props.get('mood');")
        .unwrap();
    ok(dir, &["scene", "set", "snow", "/scene/scripts", r#"[{"path": "scripts/look.js"}]"#]);
    let renamed = run(dir, &["scene", "rename-property", "snow", "mood", "weather"]);
    let warned = String::from_utf8(renamed.stderr).unwrap();
    assert!(renamed.status.success() && warned.contains("scripts/look.js names \"mood\""), "{warned}");
    let named = fails(dir, &["scene", "remove-property", "snow", "weather"]);
    assert!(named.contains("/manifest/properties/button_1/enabled_if"), "{named}");
    ok(dir, &["scene", "condition", "snow", "button_1", "", "--enabled"]);
    ok(dir, &["scene", "remove-property", "snow", "weather"]);
    assert_eq!(show(dir, "/manifest/presets/0/values"), json!({}));
    let scene = Cli::command().find_subcommand("scene").cloned().unwrap();
    for name in
        ["add-property", "move-property", "rename-property", "remove-property", "condition", "add-preset"]
    {
        let command = scene.find_subcommand(name).unwrap();
        let bare: Vec<_> =
            command.get_arguments().filter(|a| a.get_help().is_none()).map(|a| a.get_id()).collect();
        assert!(bare.is_empty(), "{name} does not explain {bare:?}");
    }
}

fn words(line: &str) -> Vec<String> {
    let (mut out, mut word, mut quote, mut started) = (Vec::new(), String::new(), None, false);
    for c in line.trim().chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => (quote, started) = (Some(c), true),
            (None, ' ') => {
                if started {
                    out.push(std::mem::take(&mut word));
                }
                started = false;
            }
            (None, c) => {
                word.push(c);
                started = true;
            }
        }
    }
    out.extend(started.then_some(word));
    out
}

#[test]
fn the_scene_commands_of_the_readme_work_as_written() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md")).unwrap();
    let elsewhere =
        ["open", "import", "add-layer", "thumbnail", "upgrade", "preview", "build", "apply", "bench"];
    let tmp = tempfile::tempdir().unwrap();
    let lines = readme.lines().filter(|l| l.starts_with("wprs-ctl scene "));
    let commands = lines.flat_map(|l| l.split("  #").next().unwrap_or_default().split(" && ")).map(words);
    let mut ran = 0;
    for args in commands.filter(|a| a[3] == "mar" && !elsewhere.contains(&a[2].as_str())) {
        ok(tmp.path(), &args[1..].iter().map(String::as_str).collect::<Vec<_>>());
        ran += 1;
    }
    assert!(ran >= 18, "only {ran} commands of the README ran");
    assert_eq!(
        show_in(tmp.path(), "mar", "/manifest/properties/velocidad/label"),
        json!({"en": "Speed", "es": "Velocidad"})
    );
}

#[test]
fn package_effects_are_imported_from_the_command_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    ok(dir, &["scene", "new", "snow", "--from", "empty", "--title", "Snow"]);
    let descriptor = json!({ "id": "dim", "cost": {}, "passes": [{ "entry": "fs_dim" }] });
    std::fs::write(dir.join("dim.json"), descriptor.to_string()).unwrap();
    std::fs::write(
        dir.join("dim.wgsl"),
        "@fragment fn fs_dim(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }",
    )
    .unwrap();
    ok(dir, &["scene", "import-effect", "snow", "dim.json"]);
    assert_eq!(show(dir, "/scene/effects/dim/wgsl"), "effects/dim.wgsl");
    std::fs::write(
        dir.join("dim.wgsl"),
        "@fragment fn fs_other(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }",
    )
    .unwrap();
    assert!(
        fails(dir, &["scene", "import-effect", "snow", "dim.json"]).contains("has no entry point fs_dim")
    );
}
