use super::note;
use serde_json::json;
use wprs_core::model::OutputState;

fn shown(reason: &str, cost: f64, auto: &str) -> OutputState {
    let state = json!({ "width": 64, "height": 32, "image": null, "fit": "fill", "kind": "scene",
        "playing": true, "active": true, "covered": false, "clients": 0, "error": null, "item": null,
        "preset": null, "playlist": null, "credit": null, "quality": "q0", "quality_reason": reason,
        "quality_cost": cost, "quality_budget": if cost > 0.0 { 4.0 } else { 0.0 }, "quality_auto": auto });
    serde_json::from_value(state).unwrap()
}

#[test]
fn a_chosen_level_on_a_scene_heavy_only_in_model_triangles_still_warns() {
    let triangles = note(&shown("user", 0.0, "stop")).unwrap();
    assert!(triangles.starts_with("this scene draws more model triangles than this device holds"));
    assert!(triangles.ends_with("automatic quality would keep it still at its poster"), "{triangles}");
    let stopped = note(&shown("device", 0.0, "stop")).unwrap();
    assert!(
        stopped.starts_with("this scene is too heavy for this device and automatic quality keeps it still")
    );
    let over = note(&shown("user", 6.0, "stop")).unwrap();
    assert!(over.starts_with("this scene costs 6.0 units a frame at q0, more than the 4"), "{over}");
    assert!(over.ends_with("automatic quality would keep it still at its poster"), "{over}");
    assert_eq!(note(&shown("user", 0.0, "")), None);
}
