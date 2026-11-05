use super::{hdr, note};
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
    for reason in ["user", "device", "slow"] {
        let triangles = note(&shown(reason, 0.0, "stop")).unwrap();
        assert!(triangles.starts_with("this scene draws more model triangles than this device holds"));
        assert!(triangles.ends_with("automatic quality would keep it still at its poster"), "{triangles}");
    }
    let stopped = note(&shown("device", 0.0, "still")).unwrap();
    assert!(
        stopped.starts_with("this scene is too heavy for this device and automatic quality keeps it still")
    );
    let over = note(&shown("user", 6.0, "stop")).unwrap();
    assert!(over.starts_with("this scene costs 6.0 units a frame at q0, more than the 4"), "{over}");
    assert!(over.ends_with("automatic quality would keep it still at its poster"), "{over}");
    assert_eq!(note(&shown("user", 0.0, "")), None);
}

#[test]
fn the_hdr_line_says_how_the_colors_reach_the_desktop() {
    let mut o = shown("user", 0.0, "");
    o.hdr = serde_json::from_value(json!({ "choice": "on", "own": null, "active": true, "offered": true,
        "display": true, "peak_nits": 800.0, "reference_nits": 203.0, "reason": "on", "encoding": "pq-own",
        "max_cll": 800, "max_fall": 203 }))
    .unwrap();
    assert_eq!(hdr(&o), "on on (on, 800 nits, PQ with its own description, MaxCLL 800)");
    o.hdr.encoding = "scrgb".into();
    assert_eq!(hdr(&o), "on on (on, 800 nits, scRGB)");
    o.hdr.active = false;
    assert_eq!(hdr(&o), "on off (on, 800 nits)");
}
