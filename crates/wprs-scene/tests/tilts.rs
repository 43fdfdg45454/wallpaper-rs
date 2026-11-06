use serde_json::json;
use std::sync::Arc;
use wprs_scene::*;

fn lean(layers: serde_json::Value, id: &str) -> Lean {
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layers });
    let rt = Runtime::new(Arc::new(Scene::from_json(&doc.to_string()).unwrap()), Default::default());
    let frame = rt.frame([0.0, 0.0, 200.0, 100.0], Quality::Q3);
    frame.draws.iter().find(|d| d.layer.id == id).and_then(|d| d.lean).unwrap_or(Lean::IDENTITY)
}

fn card(tilt: serde_json::Value) -> serde_json::Value {
    json!({ "id": "card", "type": "solid", "color": "#ffffff", "size": [40, 20], "transform": { "position": [0, 0], "tilt": tilt } })
}

fn group(tilt: [f32; 2], inner: serde_json::Value) -> serde_json::Value {
    json!([{ "id": "g", "type": "group", "transform": { "position": [100, 50], "tilt": tilt }, "layers": [inner] }])
}

const SPOTS: [[f32; 2]; 4] = [[80.0, 40.0], [120.0, 60.0], [100.0, 50.0], [90.0, 58.0]];

#[test]
fn a_tilted_layer_in_a_tilted_group_turns_with_one_rigid_camera() {
    let undone = lean(group([0.0, 40.0], card(json!([0, -40]))), "card");
    let flat = lean(group([0.0, 0.0], card(json!([0, 0]))), "card");
    for p in SPOTS {
        let (a, b) = (undone.apply(p), flat.apply(p));
        assert!((a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2, "{p:?}: {a:?} {b:?}");
    }
    let nested = lean(group([0.0, 25.0], card(json!([0, 25]))), "card");
    let once = lean(group([0.0, 50.0], card(json!([0, 0]))), "card");
    for p in SPOTS {
        let (a, b) = (nested.apply(p), once.apply(p));
        assert!((a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2, "{p:?}: {a:?} {b:?}");
    }
}
