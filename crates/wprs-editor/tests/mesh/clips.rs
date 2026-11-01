use super::kit::{act, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn clip(extra: Value) -> Value {
    let mut c = json!({ "layer": "net", "id": "wave" });
    c.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "clip": c })
}

fn key(time: f64, extra: Value) -> Value {
    let mut k = json!({ "layer": "net", "clip": "wave", "time": time });
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "key": k })
}

fn layer(p: &Project) -> &Value {
    &p.doc["scene"]["layers"][0]
}

#[test]
fn clips_are_added_renamed_timed_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, clip(json!({ "add": true, "length": 2, "loop": "pingpong" }))).unwrap();
    let seeded = json!({ "root": { "rotation": [[0, 0]] } });
    assert_eq!(layer(&p)["clips"], json!([{ "id": "wave", "to": 2.0, "loop": "pingpong", "bones": seeded }]));
    for bad in [
        clip(json!({ "add": true })),
        json!({ "clip": { "layer": "net", "id": "Big wave", "add": true } }),
        clip(json!({ "length": 0 })),
        json!({ "clip": { "layer": "net", "id": "none", "length": 1 } }),
        clip(json!({ "to": "a b" })),
    ] {
        assert!(act(&mut p, bad).is_err());
    }
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/0/animations", "value": [{ "clip": "wave" }] } }))
        .unwrap();
    act(&mut p, clip(json!({ "to": "hello", "length": 3 }))).unwrap();
    assert_eq!(
        (layer(&p)["clips"][0]["id"].clone(), layer(&p)["clips"][0]["to"].clone()),
        (json!("hello"), json!(3.0))
    );
    assert_eq!(layer(&p)["animations"], json!([{ "clip": "hello" }]));
    act(&mut p, json!({ "clip": { "layer": "net", "id": "hello", "remove": true } })).unwrap();
    assert!(layer(&p).get("clips").is_none() && layer(&p).get("animations").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(layer(&p)["animations"], json!([{ "clip": "hello" }]));
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert!(layer(&p).get("clips").is_none());
}

#[test]
fn keys_are_set_moved_copied_eased_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.9, 0.5] } }))
        .unwrap();
    act(&mut p, clip(json!({ "add": true }))).unwrap();
    act(&mut p, key(0.0, json!({ "bone": "arm", "rotation": 0 }))).unwrap();
    act(&mut p, key(1.0, json!({ "bone": "arm", "rotation": 30, "ease": "cubic" }))).unwrap();
    let arm = |p: &Project| layer(p)["clips"][0]["bones"]["arm"].clone();
    assert_eq!(arm(&p)["rotation"], json!([[0.0, 0.0], [1.0, 30.0, "in_out_cubic"]]));
    act(&mut p, key(0.5, json!({ "bone": "arm" }))).unwrap();
    assert_eq!(arm(&p)["rotation"][1], json!([0.5, 15.0]));
    assert_eq!(
        (arm(&p)["position"].clone(), arm(&p)["scale"].clone()),
        (json!([[0.5, [0, 0]]]), json!([[0.5, [1, 1]]]))
    );
    act(&mut p, key(1.5, json!({ "bone": "arm", "from": 1.0 }))).unwrap();
    act(&mut p, key(2.0, json!({ "bone": "arm", "from": 1.5, "copy": true }))).unwrap();
    assert_eq!(
        arm(&p)["rotation"].as_array().unwrap()[2..],
        [json!([1.5, 30.0, "in_out_cubic"]), json!([2.0, 30.0, "in_out_cubic"])]
    );
    act(&mut p, key(2.0, json!({ "bone": "arm", "remove": true }))).unwrap();
    act(&mut p, key(0.0, json!({ "bone": "arm", "ease": "step" }))).unwrap();
    assert_eq!(arm(&p)["rotation"][0], json!([0.0, 0.0, "step"]));
    act(&mut p, key(0.0, json!({ "order": "body", "value": 2.4 }))).unwrap();
    assert_eq!(layer(&p)["clips"][0]["order"], json!({ "body": [[0.0, 2.0]] }));
    act(&mut p, key(0.0, json!({ "order": "body", "remove": true }))).unwrap();
    assert!(layer(&p)["clips"][0].get("order").is_none());
    let rows = query(&p, "net", json!({ "keys": "wave" }));
    let first = rows["rows"].as_array().unwrap().iter().find(|r| r["channel"] == "rotation").unwrap();
    assert_eq!(
        (first["name"].clone(), first["channel"].clone(), first["keys"][0].clone()),
        (json!("arm"), json!("rotation"), json!({ "t": 0.0, "value": 0.0, "ease": "step" }))
    );
    assert_eq!((rows["length"].clone(), rows["parts"].clone()), (json!(2.0), json!(["body"])));
    for bad in [
        key(0.0, json!({ "bone": "arm", "order": "body", "value": 1 })),
        key(0.0, json!({ "bone": "tail", "rotation": 1 })),
        key(-1.0, json!({ "bone": "arm", "rotation": 1 })),
        key(0.0, json!({ "bone": "arm", "rotation": 1e6 })),
        key(3.0, json!({ "bone": "arm", "remove": true })),
        key(0.0, json!({ "shape": "smile", "weight": 1 })),
        json!({ "key": { "layer": "net", "clip": "none", "time": 0, "bone": "arm" } }),
    ] {
        assert!(act(&mut p, bad).is_err());
    }
    let before = layer(&p).clone();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_ne!(layer(&p), &before);
}

#[test]
fn the_pose_of_a_clip_puts_the_handles_where_the_bones_are() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.9, 0.5] } }))
        .unwrap();
    act(&mut p, clip(json!({ "add": true, "length": 2 }))).unwrap();
    act(&mut p, key(1.0, json!({ "bone": "arm", "rotation": 90 }))).unwrap();
    let pose = |q: Value| query(&p, "net", json!({ "pose": q }));
    let at = |shown: &Value, n: usize| shown["bones"][n]["at"].clone();
    let shown = pose(json!({ "clip": "wave", "time": 1 }));
    assert_eq!((at(&shown, 1), at(&shown, 2)), (json!([0.5, 0.5]), json!([0.5, 1.3])));
    assert_eq!(
        (shown["bones"][1]["rotation"].clone(), shown["bones"][1]["angle"].clone()),
        (json!(90.0), json!(90.0))
    );
    assert_eq!(shown["bones"][2]["frame"], json!([0.0, 1.0, -1.0, 0.0]));
    let pending =
        pose(json!({ "clip": "wave", "time": 1, "bone": "arm", "rotation": 0, "position": [8, 0] }));
    assert_eq!(at(&pending, 2), json!([1.0, 0.5]));
    assert_eq!(pending["bones"][1]["position"], json!([8.0, 0.0]));
    assert_eq!(pose(json!({ "clip": "none" })), Value::Null);
}

#[test]
fn the_preview_plays_the_chosen_clip_with_the_chosen_shapes() {
    let mut scene = json!({ "layers": [
        { "id": "net", "type": "mesh", "animations": [{ "clip": "a" }], "shapes": { "x": 0.2 } },
        { "id": "g", "type": "group", "layers": [{ "id": "inner", "type": "mesh" }] },
    ] });
    let look = json!({ "layer": "net", "clip": "b", "shapes": { "y": 0.5 } });
    wprs_editor::rigs::preview(&mut scene, &look);
    assert_eq!(scene["layers"][0]["animations"], json!([{ "clip": "b" }]));
    assert_eq!(scene["layers"][0]["shapes"], json!({ "x": 0.2, "y": 0.5 }));
    wprs_editor::rigs::preview(&mut scene, &json!({ "layer": "inner", "clip": "c" }));
    assert_eq!(scene["layers"][1]["layers"][0]["animations"], json!([{ "clip": "c" }]));
    let kept = scene.clone();
    wprs_editor::rigs::preview(&mut scene, &json!({ "layer": "g", "clip": "c" }));
    wprs_editor::rigs::preview(&mut scene, &json!({}));
    assert_eq!(scene, kept);
}
