mod language;
mod more;

use clap::CommandFactory;
use image::{Rgba, RgbaImage};
use language::show_in;
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use wprs_ctl::Cli;
use wprs_editor::{ACTIONS, Action};

pub fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null()).env("XDG_STATE_HOME", dir.join("state"));
    command.env("LC_ALL", "C");
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    command.output().unwrap()
}

pub fn ok(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

pub fn fails(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(1), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8(out.stderr).unwrap()
}

pub fn show(dir: &Path, pointer: &str) -> Value {
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
    ok(dir, &["scene", "keyframe", "snow", "/scene/layers/1/opacity", "3", "--from", "4"]);
    ok(dir, &["scene", "keyframe", "snow", "/scene/layers/1/opacity", "4", "--from", "3"]);
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
    assert_eq!(history.lines().count(), 19, "{history}");
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
    assert_eq!(ok(dir, &["scene", "history", "copy"]), "Change a value\nChange a value\n");
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

pub fn ok_lines(dir: &Path) -> String {
    String::from_utf8(run(dir, &["scene", "validate", "snow"]).stdout).unwrap()
}

pub fn fields(value: Value) -> Vec<String> {
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
