use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod events;
mod sounds;

fn runtime(timelines: Json, layers: Json) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "timelines": timelines,
        "assets": { "a": { "path": "a.ogg" }, "b": { "path": "b.ogg" }, "c": { "path": "c.ogg" } },
        "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    assert_eq!(scene.validate(&BTreeMap::new()), []);
    Runtime::new(Arc::new(scene), Values::new())
}

fn names(rt: &mut Runtime) -> Vec<String> {
    let fired = rt.take_fired().into_iter();
    fired.map(|f| format!("{}:{}", f.timeline, f.name.unwrap_or_else(|| "ended".into()))).collect()
}

fn box_x(rt: &Runtime) -> f32 {
    rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).draws[0].world.apply([0.0, 0.0])[0]
}
