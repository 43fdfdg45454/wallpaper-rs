use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

mod drawn;
mod effects;
mod timelines;

const NEWER: &str =
    "tint blend_texture light_shafts blend_gradient caustics displace electric fire fluid glitter
    lens_flare local_contrast pulse reveal spin trail transform vhs";

fn scene(more: Json) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "sky": { "path": "sky.png" } } });
    doc.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    Scene::from_json(&doc.to_string()).unwrap_or_else(|e| panic!("{more}: {e}"))
}

fn issues(more: Json) -> Vec<String> {
    scene(more).validate(&BTreeMap::new()).iter().map(ToString::to_string).collect()
}

fn effect(e: Json) -> Json {
    json!({ "layers": [{ "id": "sky", "type": "image", "image": "sky", "effects": [e] }] })
}
