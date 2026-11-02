use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

mod air;
mod cast;
mod edits;
mod gizmos;
mod maps;
mod shared;
mod surfaces;
mod swarm;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

pub fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    let action = serde_json::from_value::<Action>(action).unwrap();
    assert!(wprs_editor::ACTIONS.contains(&action.name().as_str()), "{}", action.name());
    project.act(&action).map(drop)
}

pub fn light(project: &mut Project, args: Value) -> anyhow::Result<()> {
    act(project, json!({ "light": args }))
}

pub fn layer<'a>(p: &'a Project, id: &str) -> &'a Value {
    wprs_editor::actions::layer(&p.doc, id).unwrap().1
}

pub fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("lamp");
    let mut project =
        Project::create(&dir, starter("particles", "org.example.lamp", "Lamp", "en").unwrap()).unwrap();
    let pic = RgbaImage::from_fn(40, 20, |x, _| {
        Rgba(if x < 20 { [40, 40, 40, 255] } else { [220, 220, 220, 255] })
    });
    pic.save(dir.join("pic.png")).unwrap();
    std::fs::write(dir.join("song.ogg"), b"OggS").unwrap();
    for (id, path) in [("pic", "pic.png"), ("song", "song.ogg")] {
        let pointer = format!("/scene/assets/{id}");
        act(&mut project, json!({ "set": { "pointer": pointer, "value": { "path": path } } })).unwrap();
    }
    let logo =
        json!({ "id": "logo", "type": "image", "image": "pic", "transform": { "position": [960, 540] } });
    act(&mut project, json!({ "add": { "pointer": "/scene/layers/-", "value": logo } })).unwrap();
    project
}

pub fn undone(p: &mut Project, before: &Value) {
    let after = p.doc.clone();
    act(p, json!({ "undo": {} })).unwrap();
    assert_eq!(&p.doc, before, "undo goes back");
    act(p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc, after, "redo comes again");
}
