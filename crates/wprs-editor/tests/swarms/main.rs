use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

mod edits;
mod fields;
mod handles;
mod looks;
mod tends;

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

pub fn edit(project: &mut Project, what: &str, args: Value) -> anyhow::Result<()> {
    let mut args = args;
    args["system"] = json!("flakes");
    act(project, json!({ "particles": { what: args } }))
}

pub fn system(p: &Project) -> Value {
    p.doc["scene"]["particles"]["flakes"].clone()
}

pub fn handles(p: &Project) -> Value {
    wprs_editor::swarms::handles(&p.dir, &p.doc, "flakes", None, 0.0)
}

pub fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("snow");
    let mut project =
        Project::create(&dir, starter("particles", "org.example.snow", "Snow", "en").unwrap()).unwrap();
    let pic = RgbaImage::from_fn(40, 20, |x, _| Rgba(if x < 20 { [255, 0, 0, 255] } else { [0; 4] }));
    pic.save(dir.join("pic.png")).unwrap();
    act(&mut project, json!({ "set": { "pointer": "/scene/assets/pic", "value": { "path": "pic.png" } } }))
        .unwrap();
    let layer =
        json!({ "id": "logo", "type": "image", "image": "pic", "transform": { "position": [960, 540] } });
    act(&mut project, json!({ "add": { "pointer": "/scene/layers/-", "value": layer } })).unwrap();
    project
}

pub fn undone(p: &mut Project, before: &Value) {
    let after = p.doc.clone();
    act(p, json!({ "undo": {} })).unwrap();
    assert_eq!(&p.doc, before, "undo goes back");
    act(p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc, after, "redo comes again");
}
