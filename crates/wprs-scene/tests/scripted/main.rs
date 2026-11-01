use serde_json::json;
use std::sync::Arc;
use wprs_scene::*;

mod geometry;
mod modules;
mod templates;
mod values;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

fn runtime(extra: serde_json::Value) -> Runtime {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "templates": { "bar": { "id": "bar", "type": "group", "layers": [{ "id": "fill", "type": "solid", "color": "#ffffff", "size": [4, 10] }] } },
        "layers": [
            { "id": "back", "type": "solid", "color": "#000000", "effects": [{ "effect": "blur", "name": "soft" }, { "effect": "water_ripple" }] },
            { "id": "label", "type": "text", "text": "a", "size": 12, "font": "serif" }
        ] });
    for (k, v) in extra.as_object().into_iter().flatten() {
        doc[k] = v.clone();
    }
    Runtime::new(Arc::new(Scene::from_json(&doc.to_string()).unwrap()), Values::new())
}

fn ids(rt: &Runtime) -> Vec<String> {
    rt.frame(VIEW, Quality::Q3).draws.iter().map(|d| d.layer.id.clone()).collect()
}
