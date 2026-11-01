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

pub fn animated(p: &mut Project, bone: &str, by: [f32; 3]) {
    let path = p.doc["scene"]["assets"]["net"]["path"].as_str().unwrap().to_owned();
    let bytes = std::fs::read(p.dir.join(&path)).unwrap();
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let text = word(12);
    let mut doc: Value = serde_json::from_slice(&bytes[20..20 + text]).unwrap();
    let mut bin = bytes[28 + text..28 + text + word(20 + text)].to_vec();
    let node = doc["nodes"].as_array().unwrap().iter().position(|n| n["name"] == bone).unwrap();
    let rest: Vec<f32> =
        (0..3).map(|i| doc["nodes"][node]["translation"][i].as_f64().unwrap() as f32).collect();
    let moved: Vec<f32> = (0..3).map(|i| rest[i] + by[i]).collect();
    let mut add = |values: &[f32], kind: &str| {
        let views = doc["bufferViews"].as_array_mut().unwrap();
        views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": values.len() * 4 }));
        let view = views.len() - 1;
        bin.extend(values.iter().flat_map(|v| v.to_le_bytes()));
        let count = if kind == "SCALAR" { values.len() } else { values.len() / 3 };
        let accessors = doc["accessors"].as_array_mut().unwrap();
        accessors.push(json!({ "bufferView": view, "componentType": 5126, "type": kind, "count": count }));
        accessors.len() - 1
    };
    let times = add(&[0.0, 1.0], "SCALAR");
    let values = add(&[rest, moved].concat(), "VEC3");
    doc["animations"] = json!([{ "name": "slide", "samplers": [{ "input": times, "output": values }],
        "channels": [{ "sampler": 0, "target": { "node": node, "path": "translation" } }] }]);
    doc["buffers"] = json!([{ "byteLength": bin.len() }]);
    let mut text = doc.to_string().into_bytes();
    text.resize(text.len().div_ceil(4) * 4, b' ');
    let mut out = Vec::new();
    for w in [0x4654_6C67u32, 2, (28 + text.len() + bin.len()) as u32, text.len() as u32, 0x4E4F_534A] {
        out.extend(w.to_le_bytes());
    }
    out.extend(text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004E_4942u32.to_le_bytes());
    out.extend(bin);
    std::fs::write(p.dir.join("meshes/slide.glb"), out).unwrap();
    act(p, json!({ "set": { "pointer": "/scene/assets/net/path", "value": "meshes/slide.glb" } })).unwrap();
}
