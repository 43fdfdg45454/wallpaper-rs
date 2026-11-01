use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    project.act(&serde_json::from_value::<Action>(action).unwrap()).map(drop)
}

fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("fox");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.fox", "Fox", "en").unwrap()).unwrap();
    let picture = RgbaImage::from_fn(80, 40, |x, y| {
        let inside = ((x as f32 - 40.0) / 36.0).powi(2) + ((y as f32 - 20.0) / 16.0).powi(2) < 1.0;
        Rgba([200, 120, 40, if inside { 255 } else { 0 }])
    });
    picture.save(dir.join("fox.png")).unwrap();
    act(&mut project, json!({ "set": { "pointer": "/scene/assets/fox", "value": { "path": "fox.png" } } }))
        .unwrap();
    project
}

#[test]
fn a_mesh_is_cut_from_a_picture_and_takes_bones() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(&root);
    act(&mut project, json!({ "mesh_from_picture": { "image": "fox", "outline": true, "cells": 8 } }))
        .unwrap();
    let layer = project.doc["scene"]["layers"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        (layer["type"].as_str(), layer["texture"].as_str(), layer["size"].clone()),
        (Some("mesh"), Some("fox"), json!([80, 40]))
    );
    let id = layer["id"].as_str().unwrap().to_owned();
    let info = |p: &Project| wprs_editor::rigs::rig(&p.dir, &p.doc, &id);
    assert_eq!(
        (info(&project)["editable"].clone(), info(&project)["parts"].clone()),
        (json!(true), json!(["body"]))
    );
    let bone = |name: &str, parent: Value, at: [f32; 2]| json!({ "bone": { "layer": id, "name": name, "parent": parent, "at": at } });
    act(&mut project, bone("root", Value::Null, [0.2, 0.5])).unwrap();
    act(&mut project, bone("arm", json!("root"), [0.6, 0.5])).unwrap();
    act(&mut project, bone("hand", json!("arm"), [0.9, 0.5])).unwrap();
    let bones = info(&project)["bones"].clone();
    assert_eq!(bones[1], json!({ "name": "arm", "parent": "root", "at": [0.6, 0.5] }));
    act(&mut project, json!({ "bone": { "layer": id, "name": "arm", "at": [0.5, 0.25] } })).unwrap();
    assert_eq!(info(&project)["bones"][1]["at"], json!([0.5, 0.25]));
    act(&mut project, json!({ "bone": { "layer": id, "name": "arm", "remove": true } })).unwrap();
    let bones = info(&project)["bones"].clone();
    assert_eq!((bones.as_array().unwrap().len(), bones[1]["parent"].clone()), (2, json!("root")));
    act(&mut project, json!({ "undo": {} })).unwrap();
    assert_eq!(info(&project)["bones"].as_array().unwrap().len(), 3);
    let looped = json!({ "bone": { "layer": id, "name": "root", "parent": "hand" } });
    assert!(act(&mut project, looped).is_err());
    assert!(act(&mut project, bone("a/b", Value::Null, [0.5, 0.5])).is_err());
    let clip = json!([{ "id": "wave", "bones": { "arm": { "rotation": [[0, 0], [1, 30]] } } }]);
    let at = format!("/scene/layers/{}/clips", project.doc["scene"]["layers"].as_array().unwrap().len() - 1);
    act(&mut project, json!({ "set": { "pointer": at, "value": clip } })).unwrap();
    assert_eq!(info(&project)["clips"], json!(["wave"]));
    let problems = wprs_editor::package::problems(&project.dir, &project.doc);
    assert!(problems.iter().all(|p| p.warning), "{problems:?}");
}

#[test]
fn a_grid_mesh_covers_the_picture_and_names_what_it_needs() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(&root);
    act(&mut project, json!({ "mesh_from_picture": { "image": "fox", "cells": 4, "id": "net" } })).unwrap();
    assert!(project.dir.join(project.doc["scene"]["assets"]["net"]["path"].as_str().unwrap()).exists());
    assert!(act(&mut project, json!({ "mesh_from_picture": { "image": "fox", "id": "net" } })).is_err());
    assert!(act(&mut project, json!({ "mesh_from_picture": { "image": "nothing" } })).is_err());
    assert!(act(&mut project, json!({ "bone": { "layer": "nothing", "name": "x" } })).is_err());
}

#[test]
fn chains_swings_touches_depth_and_attachments_are_set_and_inspected() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(&root);
    act(&mut project, json!({ "mesh_from_picture": { "image": "fox", "cells": 4, "id": "net" } })).unwrap();
    let bone = |name: &str, parent: Value, at: [f32; 2]| json!({ "bone": { "layer": "net", "name": name, "parent": parent, "at": at } });
    act(&mut project, bone("root", Value::Null, [0.2, 0.5])).unwrap();
    act(&mut project, bone("arm", json!("root"), [0.6, 0.5])).unwrap();
    let at = format!("/scene/layers/{}", project.doc["scene"]["layers"].as_array().unwrap().len() - 1);
    let set = |p: &mut Project, key: &str, value: Value| {
        act(p, json!({ "set": { "pointer": format!("{at}/{key}"), "value": value } }))
    };
    set(&mut project, "ik", json!([{ "chain": ["root", "arm"], "target": "cursor" }])).unwrap();
    set(&mut project, "physics", json!([{ "bone": "arm", "mode": "rope", "gravity": 1 }])).unwrap();
    set(&mut project, "hit", json!(["arm"])).unwrap();
    set(&mut project, "depth", json!({ "scale": 2, "cull": true })).unwrap();
    act(&mut project, json!({ "set": { "pointer": "/scene/canvas/wind", "value": [30, 0] } })).unwrap();
    let problems = wprs_editor::package::problems(&project.dir, &project.doc);
    assert!(problems.iter().all(|p| p.warning), "{problems:?}");
    let keys: Vec<String> =
        wprs_editor::inspect::fields(&project.doc, &at).into_iter().map(|f| f.key).collect();
    for key in ["ik", "physics", "hit", "depth", "attach"] {
        assert!(keys.iter().any(|k| k == key), "{key}: {keys:?}");
    }
    let wind =
        wprs_editor::inspect::fields(&project.doc, "/scene/canvas").into_iter().find(|f| f.key == "wind");
    assert_eq!(wind.map(|f| f.widget), Some("vec2".to_string()));
    assert!(
        set(&mut project, "ik", json!([{ "chain": ["arm", "root"], "target": "cursor" }])).is_err() || {
            let problems = wprs_editor::package::problems(&project.dir, &project.doc);
            problems.iter().any(|p| !p.warning)
        }
    );
}
