use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod flight;
mod frames;
mod limits;
mod posing;
mod reading;

pub fn cube() -> Vec<u8> {
    Forge::default()
        .material(ForgeMaterial::new("paint", [1.0; 4]))
        .part("box", ForgeShape::Cube, [0.0; 3], 2.0, 0)
        .glb()
}

pub fn scene(extra: Value, layers: Value) -> Scene {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] },
        "assets": { "toy": { "path": "toy.glb" } }, "layers": layers });
    extra.as_object().unwrap().iter().for_each(|(k, v)| doc[k] = v.clone());
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    scene.adopt(&BTreeMap::new(), |_: &str, _: usize| Ok::<_, String>(cube())).unwrap();
    scene
}

pub fn runtime(scene: Scene) -> Runtime {
    Runtime::new(Arc::new(scene), Values::new())
}

pub fn model(id: &str, extra: Value) -> Value {
    let mut l = json!({ "id": id, "type": "model", "model": "toy" });
    extra.as_object().unwrap().iter().for_each(|(k, v)| l[k] = v.clone());
    l
}
