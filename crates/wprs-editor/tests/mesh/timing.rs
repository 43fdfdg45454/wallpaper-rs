use super::kit::{act, animated, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn key(time: f64, extra: Value) -> Value {
    let mut k = json!({ "layer": "net", "clip": "wave", "time": time });
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "key": k })
}

fn arm(p: &Project) -> Value {
    p.doc["scene"]["layers"][0]["clips"][0]["bones"]["arm"].clone()
}

fn waving(root: &tempfile::TempDir, clip: Value) -> Project {
    let mut p = rigged(root);
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/clips", "value": [clip] } })).unwrap();
    p
}

#[test]
fn keys_go_where_the_playhead_is_on_clips_that_start_later() {
    let root = tempfile::tempdir().unwrap();
    let mut p = waving(
        &root,
        json!({ "id": "wave", "from": 0.25, "to": 2, "bones": { "root": { "rotation": [[0.25, 0]] } } }),
    );
    act(&mut p, key(0.5, json!({ "bone": "arm", "rotation": 30 }))).unwrap();
    assert_eq!(arm(&p)["rotation"], json!([[0.75, 30.0]]));
    let rows = query(&p, "net", json!({ "keys": "wave" }));
    let row = rows["rows"].as_array().unwrap().iter().find(|r| r["name"] == "arm").unwrap().clone();
    assert_eq!((rows["length"].clone(), row["keys"][0]["t"].clone()), (json!(1.75), json!(0.5)));
    let pose = query(&p, "net", json!({ "pose": { "clip": "wave", "time": 0.5 } }));
    assert_eq!(pose["bones"][1]["rotation"], json!(30.0));
    act(&mut p, key(1.0, json!({ "bone": "arm", "from": 0.5 }))).unwrap();
    assert_eq!(arm(&p)["rotation"], json!([[1.25, 30.0]]));
    assert!(act(&mut p, key(0.5, json!({ "bone": "arm", "remove": true }))).is_err());
    act(&mut p, key(1.0, json!({ "bone": "arm", "remove": true }))).unwrap();
    assert!(arm(&p).is_null());
}

#[test]
fn keying_a_turn_leaves_the_motion_of_the_file_and_a_full_pose_takes_it() {
    let root = tempfile::tempdir().unwrap();
    let mut p = waving(&root, json!({ "id": "wave", "animation": "slide" }));
    animated(&mut p, "arm", [10.0, 0.0, 0.0]);
    let pose = query(&p, "net", json!({ "pose": { "clip": "wave", "time": 0.5 } }));
    assert_eq!(
        (pose["bones"][1]["position"].clone(), pose["bones"][1]["rotation"].clone()),
        (json!([5.0, 0.0]), json!(0.0))
    );
    act(&mut p, key(0.5, json!({ "bone": "arm", "rotation": 30 }))).unwrap();
    assert_eq!(arm(&p), json!({ "rotation": [[0.5, 30.0]] }));
    let pose = query(&p, "net", json!({ "pose": { "clip": "wave", "time": 0.75 } }));
    assert_eq!(pose["bones"][1]["position"], json!([7.5, 0.0]));
    act(&mut p, key(0.5, json!({ "bone": "arm" }))).unwrap();
    assert_eq!(
        arm(&p),
        json!({ "rotation": [[0.5, 30.0]], "position": [[0.5, [5.0, 0.0]]], "scale": [[0.5, [1.0, 1.0]]] })
    );
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(arm(&p), json!({ "rotation": [[0.5, 30.0]] }));
}
