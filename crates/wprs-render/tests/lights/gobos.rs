use super::{Shot, UP, at, files, renderer, scene, wall};
use serde_json::{Value, json};
use wprs_scene::Quality;

fn spot(texture: Value) -> Value {
    json!({ "id": "spot", "type": "light", "light": "spot", "cone": [80, 90], "radius": 400, "z": 40,
        "intensity": 2, "transform": { "position": [80, 45], "tilt": [89, 0] }, "texture": texture })
}

fn lit(layers: Value) -> Option<image::RgbaImage> {
    let r = renderer()?;
    Some(Shot::new(r, scene(layers, json!({ "ambient": "#000000" })), &files(UP)).pixels(r, Quality::Q3))
}

#[test]
fn a_spot_projects_a_picture_onto_lit_layers() {
    let Some(plain) = lit(json!([wall(json!({})), spot(Value::Null)])) else { return };
    let Some(cast) = lit(json!([wall(json!({})), spot(json!("mask"))])) else { return };
    let (left, right) = (at(&cast, 70, 45), at(&cast, 90, 45));
    assert!(right[0] > left[0] + 60, "{left:?} {right:?}");
    assert!(at(&plain, 70, 45)[0] > left[0] + 60, "{:?}", at(&plain, 70, 45));
}

#[test]
fn a_spot_projects_a_layer_with_its_effects() {
    let pattern = json!({ "id": "pattern", "type": "shape", "shape": { "rect": { "size": [80, 90] } },
        "fill": "#ffffff", "visible": false, "transform": { "position": [120, 45] },
        "effects": [{ "effect": "tint", "params": { "color": "#ff0000" } }] });
    let layers = json!([wall(json!({})), pattern, spot(json!({ "layer": "pattern" }))]);
    let Some(cast) = lit(layers) else { return };
    let p = at(&cast, 80, 45);
    assert!(u16::from(p[2]) > u16::from(p[0]) + 40, "red, in the blue, green, red order: {p:?}");
}

#[test]
fn a_still_projected_picture_keeps_the_lit_layer_cached() {
    let Some(r) = renderer() else { return };
    let spin = json!({ "timeline": "go", "keys": [[0, 0], [4, 360]] });
    let dot = json!({ "id": "dot", "type": "shape", "shape": { "rect": { "size": [10, 10] } }, "fill": "#ff0000",
        "transform": { "position": [10, 10], "rotation": spin } });
    let layers = json!([wall(json!({ "strength": 1 })), spot(json!("mask")), dot]);
    let mut shot = Shot::new(r, scene(layers, json!({})), &files(UP));
    shot.pixels(r, Quality::Q3);
    shot.runtime.advance(0.1);
    shot.pixels(r, Quality::Q3);
    assert!(shot.view.stats().damage < 10, "{:?}", shot.view.stats());
}
