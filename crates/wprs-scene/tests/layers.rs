use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

fn runtime(canvas: Json, layers: Json) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers });
    Runtime::new(Arc::new(Scene::load(&text.to_string(), &BTreeMap::new()).unwrap()), Values::new())
}

fn box_at(id: &str, more: Json) -> Json {
    let mut layer = json!({ "id": id, "type": "solid", "color": "#ffffff", "size": [10, 10] });
    layer.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    layer
}

#[test]
fn parents_carry_children_without_changing_the_order() {
    let layers = json!([
        box_at(
            "kid",
            json!({ "parent": "dad", "inherit": ["transform", "opacity"], "opacity": 0.5,
            "transform": { "position": [10, 0] } })
        ),
        box_at(
            "dad",
            json!({ "opacity": 0.5, "transform": { "position": [40, 20], "rotation": 90, "scale": 2 } })
        ),
        box_at("hidden", json!({ "parent": "off", "transform": { "position": [5, 5] } })),
        box_at("off", json!({ "visible": false })),
        box_at(
            "free",
            json!({ "parent": "off", "inherit": ["opacity"], "transform": { "position": [5, 5] } })
        )
    ]);
    let rt = runtime(json!({ "size": [100, 50] }), layers);
    let frame = rt.frame(VIEW, Quality::Q3);
    let ids: Vec<&str> = frame.draws.iter().map(|d| d.layer.id.as_str()).collect();
    assert_eq!(ids, ["kid", "dad", "free"]);
    let [x, y] = frame.draws[0].world.apply([0.0; 2]);
    assert!((x - 40.0).abs() < 1e-3 && (y - 40.0).abs() < 1e-3, "{x} {y}");
    assert!((frame.draws[0].world.0[1] - 2.0).abs() < 1e-3 && frame.draws[0].opacity == 0.25);
    assert_eq!(frame.draws[2].world.apply([0.0; 2]), [5.0, 5.0]);
}

#[test]
fn parallax_follows_its_amount_delay_pointer_share_and_the_tilt() {
    let layers =
        json!([box_at("near", json!({ "parallax": [0.1, 0.2], "transform": { "position": [50, 25] } }))]);
    let glide =
        json!({ "size": [100, 50], "parallax": { "amount": 2, "delay": 0, "pointer": 0.5, "gyro": true } });
    let mut rt = runtime(glide, layers.clone());
    rt.advance(0.05);
    let pointer = PointerState { pos: Some([1.0; 2]), smooth: [1.0; 2], inside: true, ..Default::default() };
    rt.set_pointer(pointer);
    let at = |rt: &Runtime| rt.frame(VIEW, Quality::Q3).draws[0].world.apply([0.0; 2]);
    let [x, y] = at(&rt);
    assert!((x - 55.0).abs() < 1e-3 && (y - 30.0).abs() < 1e-3, "{x} {y}");
    assert!(rt.set_tilt([1.0, 0.0]) && !rt.set_tilt([1.0, 0.0]));
    assert!((at(&rt)[0] - 65.0).abs() < 1e-3);
    assert!(rt.set_reduced_motion(true) && !rt.set_reduced_motion(true) && !rt.set_tilt([-1.0, 0.0]));
    assert_eq!(at(&rt), [50.0, 25.0], "reduced motion keeps layers in place");
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(rt.set_reduced_motion(false) && (at(&rt)[0] - 45.0).abs() < 1e-3);
    let slow = json!({ "size": [100, 50], "parallax": { "delay": 2 } });
    let mut rt = runtime(slow, layers.clone());
    rt.advance(0.05);
    rt.set_pointer(pointer);
    assert!(at(&rt)[0] < 51.0);
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let mut rt = runtime(json!({ "size": [100, 50] }), layers);
    assert!(!rt.set_tilt([1.0, 0.0]));
    assert!(
        !runtime(json!({ "size": [100, 50] }), json!([])).set_reduced_motion(true),
        "nothing moves anyway"
    );
    rt.set_pointer(pointer);
    assert!((at(&rt)[0] - 55.0).abs() < 1e-3);
}

#[test]
fn tints_brightness_and_mask_strength_reach_the_draws() {
    let layers = json!([
        { "id": "img", "type": "image", "image": "a", "color": { "keys": [[0, "#ff0000"], [1, "#ff0000"]] },
          "brightness": 3, "mask": { "shape": { "ellipse": { "size": [5, 5] } }, "strength": 0.25 } },
        { "id": "g", "type": "group", "color": "#00ff00", "layers": [box_at("in", json!({}))] }
    ]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
        "assets": { "a": { "path": "a.png" } } });
    let rt = Runtime::new(Arc::new(Scene::load(&text.to_string(), &BTreeMap::new()).unwrap()), Values::new());
    let frame = rt.frame(VIEW, Quality::Q3);
    let d = &frame.draws[0];
    assert_eq!((d.tint.0, d.brightness, d.mask_strength), ([1.0, 0.0, 0.0, 1.0], 3.0, 0.25));
    assert!(frame.draws[1].isolate && frame.draws[1].tint.0 == [0.0, 1.0, 0.0, 1.0]);
    assert!(frame.draws[2].tint.0 == [1.0; 4] && frame.draws[2].brightness == 1.0);
}
