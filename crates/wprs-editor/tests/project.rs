use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::fs;
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::assets::prune;
use wprs_editor::package::{build, problems};
use wprs_editor::starters::starter;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

fn act(project: &mut Project, action: Value) {
    project.act(&serde_json::from_value::<Action>(action).unwrap()).unwrap();
}

#[test]
fn projects_keep_their_files_history_and_unsaved_changes() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("dawn");
    let mut project =
        Project::create(&dir, starter("parallax", "org.example.dawn", "Dawn", "en").unwrap()).unwrap();
    assert!(!project.dirty() && dir.join("scene.json").is_file() && dir.join("assets").is_dir());
    assert!(Project::create(&dir, Value::Null).is_err(), "a folder with files is never overwritten");
    act(&mut project, json!({"set": {"pointer": "/scene/canvas/background", "value": "#102030"}}));
    assert!(project.dirty());
    project.save().unwrap();
    let mut again = Project::open(&dir).unwrap();
    assert_eq!(
        (again.doc["scene"]["canvas"]["background"].as_str(), again.dirty()),
        (Some("#102030"), false)
    );
    act(&mut again, json!({"undo": {}}));
    assert_eq!(again.doc["scene"]["canvas"]["background"], "#000000");
    again.persist().unwrap();
    let mut later = Project::open(&dir).unwrap();
    assert!(later.pending.is_some() && later.doc["scene"]["canvas"]["background"] == "#102030");
    assert!(later.act(&Action::Redo {}).is_err() && later.save().is_err());
    later.recover(true).unwrap();
    assert_eq!(later.doc["scene"]["canvas"]["background"], "#000000");
    act(&mut later, json!({"redo": {}}));
    assert_eq!(later.doc["scene"]["canvas"]["background"], "#102030");
    fs::write(dir.join("scene.json"), "{}").unwrap();
    assert!(later.save().is_err(), "files changed outside the editor are not overwritten");
    let reopened = Project::open(&dir).unwrap();
    assert!(reopened.pending.is_none() && reopened.history.done.is_empty() && !reopened.dirty());
}

#[test]
fn assets_are_imported_within_limits_and_packages_build() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("snow");
    let mut project =
        Project::create(&dir, starter("particles", "org.example.snow", "Snow", "en").unwrap()).unwrap();
    let picture = root.path().join("Sky Photo.png");
    RgbaImage::from_pixel(32, 16, Rgba([10, 20, 30, 255])).save(&picture).unwrap();
    act(&mut project, json!({"import": {"file": picture}}));
    act(&mut project, json!({"import": {"file": picture, "id": "again"}}));
    assert_eq!(project.doc["scene"]["assets"]["sky_photo"]["path"], "assets/Sky Photo.png");
    assert_eq!(project.doc["scene"]["assets"]["again"]["path"], "assets/Sky Photo.png");
    act(&mut project, json!({"add_layer": {"kind": "image", "source": "sky_photo", "at": 1}}));
    let text = root.path().join("notes.txt");
    fs::write(&text, "x").unwrap();
    let refused = project.act(&serde_json::from_value(json!({"import": {"file": text}})).unwrap());
    assert!(refused.unwrap_err().to_string().contains("only pictures"));
    let noise = root.path().join("noise.wav");
    fs::write(&noise, b"RIFF....WAVEjunk").unwrap();
    assert!(project.act(&serde_json::from_value(json!({"import": {"file": noise}})).unwrap()).is_err());
    assert!(build(&mut project, &root.path().join("x.wpak"), None).is_err(), "unsaved scenes are not built");
    project.save().unwrap();
    fs::write(dir.join("assets/old.png"), b"x").unwrap();
    let found = problems(&project.dir, &project.doc);
    assert!(found.iter().all(|p| p.warning) && found.iter().any(|p| p.message.contains("assets/old.png")));
    assert_eq!(prune(&project.dir, &project.doc).unwrap(), ["assets/old.png"]);
    let out = root.path().join("snow.wpak");
    let (sha256, size) = build(&mut project, &out, None).unwrap();
    assert!(sha256.len() == 64 && size > 0);
    let mut package = wprs_format::open(&out, &wprs_format::Limits::default()).unwrap();
    assert_eq!(package.scene().unwrap().layers[1].id, "image");
    act(&mut project, json!({"set": {"pointer": "/scene/layers/1/opacity", "value": 7}}));
    project.save().unwrap();
    let error = build(&mut project, &out, None).unwrap_err().to_string();
    assert!(error.starts_with("/scene/layers/1/opacity:"), "{error}");
}
