use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn project(root: &TempDir) -> Project {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
    let mut p = Project::create(
        &root.path().join("lights"),
        starter("empty", "org.example.lights", "Lights", "en").unwrap(),
    )
    .unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "adjustment", "id": "glow" } })).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "box" } })).unwrap();
    p
}

fn act(p: &mut Project, action: Value) -> anyhow::Result<()> {
    p.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn layer(p: &Project) -> Value {
    p.doc["scene"]["layers"].as_array().unwrap().iter().find(|l| l["id"] == "glow").unwrap().clone()
}

#[test]
fn an_adjustment_layer_sends_to_rgb_lighting_with_its_permission_in_one_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "rgb": { "layer": "glow", "on": true, "size": [640, 80] } })).unwrap();
    assert_eq!((layer(&p)["rgb"].clone(), layer(&p)["size"].clone()), (json!(true), json!([640.0, 80.0])));
    assert_eq!(p.doc["manifest"]["permissions"]["rgb"], json!(true));
    assert_eq!(layer(&p)["transform"]["position"], json!([960.0, 540.0]));
    act(&mut p, json!({ "rgb": { "layer": "glow", "whole": true } })).unwrap();
    assert!(layer(&p).get("size").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(layer(&p)["size"], json!([640.0, 80.0]));
    act(&mut p, json!({ "redo": {} })).unwrap();
    act(&mut p, json!({ "rgb": { "layer": "glow", "on": false } })).unwrap();
    assert_eq!(layer(&p)["rgb"], json!(false));
    assert!(p.doc["manifest"].get("permissions").is_none(), "{}", p.doc["manifest"]);
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["manifest"]["permissions"]["rgb"], json!(true));
}

#[test]
fn bad_rgb_changes_are_refused_and_change_nothing() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    for bad in [
        json!({ "rgb": { "layer": "box", "on": true } }),
        json!({ "rgb": { "layer": "nothing", "on": true } }),
        json!({ "rgb": { "layer": "glow" } }),
        json!({ "rgb": { "layer": "glow", "size": [0, 10] } }),
        json!({ "rgb": { "layer": "glow", "size": [10, 100000] } }),
    ] {
        assert!(act(&mut p, bad.clone()).is_err(), "{bad}");
    }
    assert!(serde_json::from_value::<Action>(json!({ "rgb": { "layer": "glow", "colour": 1 } })).is_err());
    assert_eq!(p.doc, before);
}
