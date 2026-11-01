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

pub fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    let action = serde_json::from_value::<Action>(action).unwrap();
    assert!(wprs_editor::ACTIONS.contains(&action.name().as_str()), "{}", action.name());
    project.act(&action).map(drop)
}

pub fn info(p: &Project, layer: &str) -> Value {
    wprs_editor::rigs::rig(&p.dir, &p.doc, layer)
}

pub fn query(p: &Project, layer: &str, q: Value) -> Value {
    wprs_editor::rigs::query(&p.dir, &p.doc, layer, &q)
}

pub fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("fox");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.fox", "Fox", "en").unwrap()).unwrap();
    RgbaImage::from_pixel(80, 40, Rgba([200, 120, 40, 255])).save(dir.join("fox.png")).unwrap();
    act(&mut project, json!({ "set": { "pointer": "/scene/assets/fox", "value": { "path": "fox.png" } } }))
        .unwrap();
    project
}

pub fn rigged(root: &TempDir) -> Project {
    let mut p = project(root);
    act(&mut p, json!({ "mesh_from_picture": { "image": "fox", "cells": 8, "id": "net" } })).unwrap();
    for (name, parent, at) in [("root", Value::Null, [0.1, 0.5]), ("arm", json!("root"), [0.5, 0.5])] {
        act(&mut p, json!({ "bone": { "layer": "net", "name": name, "parent": parent, "at": at } })).unwrap();
    }
    p
}

pub fn parts(root: &TempDir) -> Project {
    let mut p = project(root);
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/parts.glb");
    std::fs::copy(fixture, p.dir.join("body.glb")).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/assets/body", "value": { "path": "body.glb" } } }))
        .unwrap();
    let layer = json!({ "id": "body", "type": "mesh", "mesh": "body", "size": [100, 100] });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": layer } })).unwrap();
    p
}
