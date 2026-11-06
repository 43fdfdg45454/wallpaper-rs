use super::{act, at, project, refused};
use serde_json::json;

#[test]
fn a_layer_is_placed_blended_and_sized_with_checks() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let path = wprs_editor::layers::find(&p.doc, "card").unwrap();
    let place = json!({"transform": {"layer": "card", "position": [100, 50], "rotation": 15, "scale": [2, 1], "size": [300, 200]}});
    act(&mut p, json!({ "layer": place })).unwrap();
    let t = at(&p, &format!("{path}/transform"));
    assert_eq!(
        (&t["position"], &t["rotation"], &t["scale"]),
        (&json!([100.0, 50.0]), &json!(15.0), &json!([2.0, 1.0]))
    );
    assert_eq!(at(&p, &format!("{path}/size")), json!([300.0, 200.0]));
    act(&mut p, json!({"layer": {"transform": {"layer": "card", "origin": [0.5, 1]}}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/transform/position")), json!([100.0, 50.0]));
    assert!(
        refused(&mut p, json!({"layer": {"transform": {"layer": "card", "rotation": 900}}}))
            .contains("rotation")
    );
    act(&mut p, json!({"add_layer": {"kind": "text", "id": "words"}})).unwrap();
    let words = refused(&mut p, json!({"layer": {"transform": {"layer": "words", "size": [10, 10]}}}));
    assert!(words.contains("text"), "{words}");
    act(&mut p, json!({"layer": {"blend": {"layer": "card", "blend": "screen"}}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/blend")), "screen");
    assert!(
        serde_json::from_value::<wprs_editor::Action>(
            json!({"layer": {"blend": {"layer": "card", "blend": "glitter"}}})
        )
        .is_err()
    );
}

#[test]
fn a_particles_layer_changes_its_instance_of_the_system() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"add_layer": {"kind": "particles", "id": "dust"}})).unwrap();
    let path = wprs_editor::layers::find(&p.doc, "dust").unwrap();
    let values = json!({"alpha": 0.5, "size": 2, "count": 0.25, "time_scale": 3, "color": "#ffcc88"});
    act(&mut p, json!({"layer": {"instance": {"layer": "dust", "values": values}}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/instance/count")), json!(0.25));
    act(&mut p, json!({"layer": {"instance": {"layer": "dust", "values": {"count": null}}}})).unwrap();
    assert!(at(&p, &format!("{path}/instance")).get("count").is_none());
    assert!(
        refused(&mut p, json!({"layer": {"instance": {"layer": "dust", "values": {"alpha": 4}}}}))
            .contains("alpha")
    );
    assert!(
        refused(&mut p, json!({"layer": {"instance": {"layer": "dust", "values": {"glow": 1}}}}))
            .contains("glow")
    );
    assert!(
        refused(&mut p, json!({"layer": {"instance": {"layer": "card", "values": {"alpha": 1}}}}))
            .contains("particles")
    );
    act(&mut p, json!({"layer": {"instance": {"layer": "dust", "clear": true}}})).unwrap();
    assert_eq!(at(&p, &format!("{path}/instance")), serde_json::Value::Null);
}

#[test]
fn the_canvas_glows_and_moves_in_depth_from_its_defaults() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"canvas": {"bloom": {"strength": 2, "hdr": [2, 1]}}})).unwrap();
    let bloom = at(&p, "/scene/canvas/bloom");
    assert_eq!(
        (&bloom["strength"], &bloom["threshold"], &bloom["hdr"]),
        (&json!(2.0), &json!(0.8), &json!({"threshold": 2, "strength": 1}))
    );
    act(&mut p, json!({"canvas": {"bloom": {"hdr": "none"}}})).unwrap();
    assert!(at(&p, "/scene/canvas/bloom").get("hdr").is_none());
    assert!(refused(&mut p, json!({"canvas": {"bloom": {"radius": 500}}})).contains("radius"));
    act(&mut p, json!({"canvas": {"bloom": {"off": true}}})).unwrap();
    assert_eq!(at(&p, "/scene/canvas/bloom"), serde_json::Value::Null);
    act(&mut p, json!({"canvas": {"parallax": {"amount": 2, "gyro": true}}})).unwrap();
    assert_eq!(
        at(&p, "/scene/canvas/parallax"),
        json!({"amount": 2.0, "delay": 0.15, "pointer": 1.0, "gyro": true})
    );
    assert!(refused(&mut p, json!({"canvas": {"parallax": {"delay": 9}}})).contains("delay"));
    act(&mut p, json!({"canvas": {"parallax": {"off": true}}})).unwrap();
    assert_eq!(at(&p, "/scene/canvas/parallax"), serde_json::Value::Null);
}

#[test]
fn a_transform_over_keyframes_is_refused_unless_it_replaces_them() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let key = |t: f64, v: [f64; 2]| json!({"keyframe": {"pointer": "/scene/layers/0/transform/position", "time": t, "value": v}});
    act(&mut p, key(0.0, [0.0, 0.0])).unwrap();
    act(&mut p, key(2.0, [100.0, 0.0])).unwrap();
    let place = |replace: bool| json!({"layer": {"transform": {"layer": "card", "position": [50, 50], "replace": replace}}});
    let words = refused(&mut p, place(false));
    assert!(words.contains("another source"), "{words}");
    act(&mut p, json!({"layer": {"transform": {"layer": "card", "rotation": 10}}})).unwrap();
    assert!(at(&p, "/scene/layers/0/transform/position/keys").is_array());
    act(&mut p, place(true)).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/transform/position"), json!([50.0, 50.0]));
}
