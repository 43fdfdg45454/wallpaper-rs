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

#[test]
fn language_files_and_package_effects_belong_to_the_package() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("tide");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.tide", "Tide", "en").unwrap()).unwrap();
    fs::create_dir(dir.join("locale")).unwrap();
    fs::write(dir.join("locale/en.json"), json!({"title": "Tide"}).to_string()).unwrap();
    let descriptor = json!({"id": "ripple", "passes": [{"entry": "fs_ripple"}], "cost": {},
        "options": [{"id": "taps", "values": [1, 2], "default": 1}]});
    fs::write(dir.join("assets/ripple.json"), descriptor.to_string()).unwrap();
    let wgsl = "override taps: i32 = 1;\n@fragment\nfn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {\n    return tap(in.uv) * f32(taps);\n}\n";
    fs::write(dir.join("assets/ripple.wgsl"), wgsl).unwrap();
    act(&mut project, json!({"set": {"pointer": "/manifest/title", "value": "@title"}}));
    let effects = json!({"ripple": {"descriptor": "assets/ripple.json", "wgsl": "assets/ripple.wgsl"}});
    act(&mut project, json!({"set": {"pointer": "/scene/effects", "value": effects}}));
    let layers =
        json!([{"id": "sea", "type": "solid", "color": "#103050", "effects": [{"effect": "ripple"}]}]);
    act(&mut project, json!({"set": {"pointer": "/scene/layers", "value": layers}}));
    let listed = |p: &Project| p.doc["manifest"]["requires"]["features"].clone();
    assert_eq!(listed(&project), json!(["custom_effects", "effect_options"]));
    let unread = problems(&project.dir, &project.doc);
    assert!(unread.iter().any(|p| p.warning && p.message.starts_with("locale/en.json ")), "{unread:?}");
    let typed = json!({"add": {"pointer": "/manifest/requires/features/-", "value": "rgb"}});
    assert!(project.act(&serde_json::from_value::<Action>(typed).unwrap()).is_err());
    act(&mut project, json!({"add": {"pointer": "/manifest/requires/features/-", "value": "locale_files"}}));
    assert_eq!(listed(&project), json!(["custom_effects", "effect_options", "locale_files"]));
    act(&mut project, json!({"set": {"pointer": "/scene/layers/0/opacity", "value": 0.5}}));
    assert_eq!(listed(&project), json!(["custom_effects", "effect_options", "locale_files"]));
    project.save().unwrap();
    let found = problems(&project.dir, &project.doc);
    let (manifest, facts) = wprs_format::inspect(&project.dir).unwrap();
    let checked: Vec<(bool, String)> = wprs_format::Platform::ALL
        .into_iter()
        .filter_map(|p| {
            let refused = wprs_format::features::playable(&manifest, &facts, p).err()?;
            Some((true, format!("{}: refused: {refused}", p.name())))
        })
        .collect();
    assert_eq!(
        found.iter().map(|p| (p.warning, p.message.clone())).collect::<Vec<_>>(),
        checked,
        "as pack check"
    );
    assert_eq!(prune(&project.dir, &project.doc).unwrap(), Vec::<String>::new());
    assert!(dir.join("assets/ripple.wgsl").is_file() && dir.join("assets/ripple.json").is_file());
    build(&mut project, &root.path().join("tide.wpak"), None).unwrap();
    let shader = dir.join("assets/ripple.wgsl");
    let modified = fs::metadata(&shader).unwrap().modified().unwrap();
    fs::write(&shader, format!("{:<1$}", "fn broken(", wgsl.len())).unwrap();
    fs::File::options().write(true).open(&shader).unwrap().set_modified(modified).unwrap();
    let unchanged = problems(&project.dir, &project.doc);
    assert_eq!(unchanged, found, "files are read again only when they change");
    fs::write(dir.join("assets/ripple.wgsl"), "fn broken(").unwrap();
    let broken = problems(&project.dir, &project.doc);
    assert!(broken.iter().any(|p| p.pointer == "/scene/effects/ripple" && !p.warning), "{broken:?}");
}

#[test]
fn scripts_that_declare_nothing_are_problems_when_they_use_the_newer_interface() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("orbit");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.orbit", "Orbit", "en").unwrap()).unwrap();
    fs::create_dir(dir.join("scripts")).unwrap();
    fs::write(dir.join("scripts/a.js"), "import { shared } from 'wprs'; export function update() {}")
        .unwrap();
    act(&mut project, json!({"set": {"pointer": "/scene/scripts", "value": [{"path": "scripts/a.js"}]}}));
    let found = problems(&project.dir, &project.doc);
    let at =
        found.iter().find(|p| p.pointer == "/scene/scripts/0").expect("the undeclared script is a problem");
    assert!(!at.warning && at.message.contains("uses shared"), "{at:?}");
    act(&mut project, json!({"set": {"pointer": "/scene/scripts/0/uses", "value": ["shared"]}}));
    let found = problems(&project.dir, &project.doc);
    assert!(found.iter().all(|p| p.pointer != "/scene/scripts/0"), "{found:?}");
}

#[test]
fn scenes_too_heavy_for_older_gpus_at_the_middle_level_get_a_warning() {
    let root = tempfile::tempdir().unwrap();
    state();
    let dir = root.path().join("haze");
    let mut project =
        Project::create(&dir, starter("parallax", "org.example.haze", "Haze", "en").unwrap()).unwrap();
    let light = problems(&project.dir, &project.doc);
    assert!(!light.iter().any(|p| p.message.contains("older integrated")), "{light:?}");
    let blur = |i: usize| json!({"id": format!("b{i}"), "type": "adjustment", "effects": [{"effect": "blur", "params": {"radius": 30}}]});
    let layers: Vec<Value> = (0..8).map(blur).collect();
    act(&mut project, json!({"set": {"pointer": "/scene/layers", "value": layers}}));
    let heavy = problems(&project.dir, &project.doc);
    let warned = heavy.iter().find(|p| p.message.contains("older integrated and phone GPUs"));
    assert!(warned.is_some_and(|p| p.warning && p.pointer == "/scene/layers"), "{heavy:?}");
}
