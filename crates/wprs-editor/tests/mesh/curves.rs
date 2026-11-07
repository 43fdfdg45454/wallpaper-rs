use super::kit::{act, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn key(time: f64, extra: Value) -> Value {
    let mut k = json!({ "layer": "net", "clip": "wave", "time": time, "bone": "arm" });
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "key": k })
}

fn rotation(p: &Project) -> Value {
    p.doc["scene"]["layers"][0]["clips"][0]["bones"]["arm"]["rotation"].clone()
}

fn keyed() -> (tempfile::TempDir, Project) {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "clip": { "layer": "net", "id": "wave", "add": true, "length": 4 } })).unwrap();
    act(&mut p, key(0.0, json!({ "rotation": 0 }))).unwrap();
    act(&mut p, key(2.0, json!({ "rotation": 40 }))).unwrap();
    (root, p)
}

#[test]
fn keys_take_every_easing_and_a_cubic_bezier() {
    let (_root, mut p) = keyed();
    act(&mut p, key(0.0, json!({ "rotation": 0, "ease": [0.25, 0.1, 0.25, 1.0] }))).unwrap();
    assert_eq!(rotation(&p)[0], json!([0.0, 0.0, [0.25, 0.1, 0.25, 1.0]]));
    assert!(serde_json::from_value::<wprs_scene::Scene>(p.doc["scene"].clone()).is_ok());
    act(&mut p, key(0.0, json!({ "rotation": 0, "ease": "out_bounce" }))).unwrap();
    assert_eq!(rotation(&p)[0], json!([0.0, 0.0, "out_bounce"]));
    act(&mut p, key(0.0, json!({ "rotation": 0, "ease": "hold" }))).unwrap();
    assert_eq!(rotation(&p)[0], json!([0.0, 0.0, "hold"]));
    act(&mut p, key(0.0, json!({ "rotation": 0, "ease": "linear" }))).unwrap();
    assert_eq!(rotation(&p)[0], json!([0.0, 0.0]));
    for bad in ["cubic", "bouncy"] {
        assert!(act(&mut p, key(0.0, json!({ "rotation": 0, "ease": bad }))).is_err());
    }
    assert!(act(&mut p, key(0.0, json!({ "rotation": 0, "ease": [0.1, 0.2] }))).is_err());
}

#[test]
fn handles_bend_one_channel_and_travel_with_the_key() {
    let (_root, mut p) = keyed();
    act(&mut p, key(0.0, json!({ "rotation": 0, "out_handle": [0.5, 60] }))).unwrap();
    act(&mut p, key(2.0, json!({ "rotation": 40, "in_handle": [0.5, -20] }))).unwrap();
    assert_eq!(rotation(&p)[0], json!([0.0, 0.0, { "out": [0.5, 60.0] }]));
    assert_eq!(rotation(&p)[1], json!([2.0, 40.0, { "in": [0.5, -20.0] }]));
    act(&mut p, key(3.0, json!({ "rotation": 10 }))).unwrap();
    act(&mut p, key(2.0, json!({ "rotation": 40, "out_handle": [1.0, 5] }))).unwrap();
    assert_eq!(rotation(&p)[1][2], json!({ "in": [0.5, -20.0], "out": [1.0, 5.0] }));
    assert!(act(&mut p, key(2.5, json!({ "from": 2.0 }))).is_err());
    act(&mut p, key(1.5, json!({ "from": 2.0 }))).unwrap();
    assert_eq!(rotation(&p)[1], json!([1.5, 40.0, { "in": [0.5, -20.0], "out": [1.0, 5.0] }]));
    assert!(serde_json::from_value::<wprs_scene::Scene>(p.doc["scene"].clone()).is_ok());
    let rows = query(&p, "net", json!({ "keys": "wave" }));
    let row = rows["rows"].as_array().unwrap().iter().find(|r| r["channel"] == "rotation").unwrap();
    assert_eq!(row["keys"][0]["ease"], json!({ "out": [0.5, 60.0] }));
    let mid = wprs_editor::keys::sample(rotation(&p).as_array().unwrap(), 0.8).unwrap();
    assert!((mid.as_f64().unwrap() - 20.0).abs() > 1e-3);
}

#[test]
fn handles_are_refused_where_they_make_no_sense() {
    let (_root, mut p) = keyed();
    for bad in [
        json!({ "rotation": 0, "out_handle": [0.5, 1], "ease": "step" }),
        json!({ "rotation": 0, "out_handle": [-1, 1] }),
        json!({ "rotation": 0, "out_handle": [2.5, 1] }),
        json!({ "rotation": 0, "position": [1, 2], "out_handle": [0.5, 0.2] }),
        json!({ "out_handle": [0.5, 0.2] }),
    ] {
        assert!(act(&mut p, key(0.0, bad)).is_err());
    }
    assert_eq!(rotation(&p), json!([[0.0, 0.0], [2.0, 40.0]]));
}
