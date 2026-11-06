use serde_json::json;
use std::sync::Arc;
use wprs_scene::{PointerState, Quality, Runtime, SCHEMA, Scene};

fn runtime(params: serde_json::Value) -> Runtime {
    let fx = json!([{ "effect": "depth_parallax", "params": params }, { "effect": "waves" }]);
    let layer = json!({ "id": "a", "type": "solid", "color": "#ff0000", "effects": fx });
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 100] }, "layers": [layer] });
    Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
}

fn effects(rt: &Runtime) -> Vec<String> {
    let frame = rt.frame([0.0, 0.0, 100.0, 100.0], Quality::Q3);
    frame.effects.iter().map(|e| e.descriptor.id.clone()).collect()
}

#[test]
fn depth_parallax_is_left_out_of_frames_where_it_would_not_move_the_picture() {
    let mut rt = runtime(json!({}));
    assert_eq!(effects(&rt), ["waves"]);
    let touch = PointerState {
        pos: Some([0.8, 0.5]),
        smooth: [0.8, 0.5],
        inside: true,
        presence: 1.0,
        ..Default::default()
    };
    rt.set_pointer(touch);
    assert_eq!(effects(&rt), ["depth_parallax", "waves"]);
    rt.set_pointer(PointerState { smooth: [0.5, 0.5], ..touch });
    assert_eq!(effects(&rt), ["waves"]);
    for params in [json!({ "orbit": true }), json!({ "overscan": 1.1 })] {
        assert_eq!(effects(&runtime(params)), ["depth_parallax", "waves"]);
    }
    let mut weak = runtime(json!({ "strength": 0 }));
    weak.set_pointer(touch);
    assert_eq!(effects(&weak), ["waves"]);
}

#[test]
fn a_glow_without_intensity_is_left_out_of_frames() {
    let fx = |intensity: f32| json!([{ "effect": "bloom", "params": { "intensity": intensity } }]);
    let layer = |i: f32| json!({ "id": "a", "type": "solid", "color": "#ff0000", "effects": fx(i) });
    let rt = |i: f32| {
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 100] }, "layers": [layer(i)] });
        Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
    };
    assert!(effects(&rt(0.0)).is_empty());
    assert_eq!(effects(&rt(0.5)), ["bloom"]);
}
