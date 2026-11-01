use super::kit::{act, info, parts};
use serde_json::json;

#[test]
fn expressions_are_named_sets_of_blend_shape_weights() {
    let root = tempfile::tempdir().unwrap();
    let mut p = parts(&root);
    assert_eq!(
        (info(&p, "body")["shapes"].clone(), info(&p, "body")["defaults"].clone()),
        (json!(["smile"]), json!([0.0]))
    );
    let set = |name: &str, w: f32| json!({ "expression": { "layer": "body", "name": name, "shapes": { "smile": w } } });
    act(&mut p, set("happy", 0.8)).unwrap();
    let layer = |p: &wprs_editor::Project| p.doc["scene"]["layers"][0].clone();
    assert_eq!(layer(&p)["expressions"], json!({ "happy": { "smile": 0.8 } }));
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/shapes", "value": { "happy": 1 } } })).unwrap();
    act(&mut p, json!({ "expression": { "layer": "body", "name": "happy", "to": "glad" } })).unwrap();
    assert_eq!(
        (layer(&p)["shapes"].clone(), info(&p, "body")["expressions"].as_object().unwrap().len()),
        (json!({ "glad": 1 }), 1)
    );
    act(&mut p, json!({ "expression": { "layer": "body", "name": "glad", "remove": true } })).unwrap();
    assert!(layer(&p).get("expressions").is_none() && layer(&p).get("shapes").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(layer(&p)["shapes"], json!({ "glad": 1 }));
    act(&mut p, json!({ "redo": {} })).unwrap();
    for bad in [
        set("sad", 2.0),
        json!({ "expression": { "layer": "body", "name": "sad", "shapes": { "frown": 1 } } }),
        json!({ "expression": { "layer": "body", "name": "", "shapes": { "smile": 1 } } }),
        json!({ "expression": { "layer": "body", "name": "nobody", "remove": true } }),
    ] {
        assert!(act(&mut p, bad).is_err());
    }
    act(&mut p, json!({ "clip": { "layer": "body", "id": "grin", "add": true } })).unwrap();
    act(
        &mut p,
        json!({ "key": { "layer": "body", "clip": "grin", "time": 0.5, "shape": "smile", "weight": 0.5 } }),
    )
    .unwrap();
    assert_eq!(layer(&p)["clips"][0]["shapes"], json!({ "smile": [[0.5, 0.5]] }));
}
