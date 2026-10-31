use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod clips;
mod layers;

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("../props.json")).unwrap()
}

fn scene(sounds: Json) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": sounds,
        "assets": { "wind": { "path": "wind.ogg" }, "pic": { "path": "pic.png" } },
        "timelines": [{ "id": "storm", "duration": 10, "loop": "repeat" }] });
    Scene::from_json(&text.to_string()).unwrap()
}

fn issues(layer: Json) -> Vec<Issue> {
    let mut layer = layer;
    layer["id"] = json!("s");
    layer["type"] = json!("sound");
    layer.as_object_mut().unwrap().entry("sound").or_insert(json!("wind"));
    scene(json!([layer])).validate(&props())
}
