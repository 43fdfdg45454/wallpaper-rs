use super::kit::{close, layers};
use serde_json::json;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

fn zoomed(attach: serde_json::Value, camera: serde_json::Value) -> [f32; 6] {
    let rt = layers(json!([
        { "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
          "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }], "animations": [{ "clip": "wave" }] },
        { "id": "badge", "type": "solid", "color": "#ffffff", "size": [4, 4], "attach": attach }
    ]));
    let mut scene = (*rt.scene).clone();
    scene.canvas.camera = Some(serde_json::from_value(camera).unwrap());
    let mut rt = Runtime::new(std::sync::Arc::new(scene), Default::default());
    rt.advance(2.0);
    rt.frame(VIEW, Quality::Q3).draws.iter().find(|d| d.layer.id == "badge").unwrap().world.0
}

#[test]
fn an_attached_layer_keeps_the_zoom_and_turn_of_the_camera_without_the_scale_of_its_bone() {
    let camera = json!({ "zoom": 2 });
    let w = zoomed(
        json!({ "layer": "fox", "bone": "hand", "inherit": ["position", "rotation"] }),
        camera.clone(),
    );
    assert!(close([w[4], w[5]], [32.0, -16.0], 1e-3), "{w:?}");
    assert!(close([w[0], w[1]], [0.0, -2.0], 1e-4) && close([w[2], w[3]], [2.0, 0.0], 1e-4), "{w:?}");
    let w = zoomed(json!({ "layer": "fox", "bone": "hand", "inherit": ["position"] }), camera.clone());
    assert!(close([w[0], w[1]], [2.0, 0.0], 1e-4) && close([w[2], w[3]], [0.0, 2.0], 1e-4), "{w:?}");
    let all = zoomed(json!({ "layer": "fox", "bone": "hand" }), camera);
    let w = zoomed(
        json!({ "layer": "fox", "bone": "hand", "inherit": ["position", "rotation"] }),
        json!({ "zoom": 2 }),
    );
    assert!(
        all.iter().zip(&w).all(|(a, b)| (a - b).abs() < 1e-3),
        "a bone of scale 1 adds nothing: {all:?} {w:?}"
    );
    let turned =
        zoomed(json!({ "layer": "fox", "bone": "hand", "inherit": ["position"] }), json!({ "rotation": 90 }));
    assert!(close([turned[0], turned[1]], [0.0, 1.0], 1e-4), "the camera still turns it: {turned:?}");
    let still =
        zoomed(json!({ "layer": "fox", "bone": "hand", "inherit": ["rotation"] }), json!({ "zoom": 2 }));
    assert!(
        close([still[4], still[5]], [32.0, 32.0], 1e-3),
        "without position it stays on the origin of the mesh: {still:?}"
    );
}
