use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod forms;
mod motion;
mod values;

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("../props.json")).unwrap()
}

fn runtime(text: &str) -> Runtime {
    let props = props();
    let scene = Scene::load(text, &props).unwrap();
    Runtime::new(Arc::new(scene), effective(&props, &[]))
}

fn scene(layers: serde_json::Value) -> String {
    json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
        "timelines": [{ "id": "intro", "duration": 2.0, "delay": 1.0 }],
        "assets": { "a": { "path": "a.png" }, "b": { "path": "b.png" } } })
    .to_string()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];
