use super::{act, edit, project, system};
use serde_json::json;

#[test]
fn a_system_is_created_without_a_layer_and_started_from_another() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let create = json!({"create": {"system": "sparks", "parent": "flakes", "on": "death"}});
    act(&mut p, json!({ "particles": create })).unwrap();
    assert_eq!(system(&p)["children"], json!([{"system": "sparks", "on": "death"}]));
    assert!(p.doc["scene"]["particles"]["sparks"]["emitters"].is_array());
    act(&mut p, json!({"particles": {"create": {"system": "embers", "from": "sparks"}}})).unwrap();
    assert_eq!(p.doc["scene"]["particles"]["embers"], p.doc["scene"]["particles"]["sparks"]);
    assert!(act(&mut p, json!({"particles": {"create": {"system": "sparks"}}})).is_err());
    assert!(act(&mut p, json!({"particles": {"create": {"system": "Bad Id"}}})).is_err());
    assert!(act(&mut p, json!({"particles": {"delete": {"system": "sparks"}}})).is_err());
    act(&mut p, json!({"particles": {"delete": {"system": "embers"}}})).unwrap();
    assert!(p.doc["scene"]["particles"].get("embers").is_none());
}

#[test]
fn settings_and_starting_ranges_are_set_and_checked() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let bound = edit(&mut p, "settings", json!({"rate": 40})).unwrap_err().to_string();
    assert!(bound.contains("another source"), "{bound}");
    assert_eq!(system(&p)["emitters"][0]["rate"], json!({"prop": "density"}));
    let settings =
        json!({"capacity": 500, "seed": 7, "prewarm": 2, "space": "world", "rate": 40, "replace": true});
    edit(&mut p, "settings", settings).unwrap();
    let s = system(&p);
    assert_eq!((&s["capacity"], &s["seed"], &s["space"]), (&json!(500), &json!(7), &json!("world")));
    assert!(s["emitters"].as_array().unwrap().iter().all(|e| e["rate"] == json!(40.0)));
    assert!(edit(&mut p, "settings", json!({"rate": 5, "emitter": 9})).is_err());
    assert!(edit(&mut p, "settings", json!({"prewarm": 900})).is_err());
    let ranges = json!({"life": [1, 2], "size": [4, 9], "color": ["#ff0000", "#00ff00"], "alpha": [0.2, 0.8], "velocity": [[-5, 0], [5, 10]]});
    edit(&mut p, "start", json!({ "ranges": ranges })).unwrap();
    assert_eq!(system(&p)["init"]["velocity"], json!([[-5, 0], [5, 10]]));
    edit(&mut p, "start", json!({"clear": ["alpha", "velocity"]})).unwrap();
    assert!(system(&p)["init"].get("alpha").is_none() && system(&p)["init"]["size"] == json!([4, 9]));
    assert!(edit(&mut p, "start", json!({"ranges": {"wobble": [0, 1]}})).is_err());
    assert!(edit(&mut p, "start", json!({"clear": ["life"]})).is_err());
    assert!(edit(&mut p, "start", json!({"ranges": {"alpha": [0, 3]}})).is_err());
    assert!(edit(&mut p, "start", json!({"ranges": {"color": ["red", "#00ff00"]}})).is_err());
}

#[test]
fn children_curves_and_the_render_look_are_edited() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"particles": {"create": {"system": "sparks"}}})).unwrap();
    edit(&mut p, "child", json!({"child": {"system": "sparks", "on": "static", "scale": 2}})).unwrap();
    edit(&mut p, "child", json!({"index": 0, "child": {"system": "sparks", "on": "spawn"}})).unwrap();
    assert_eq!(system(&p)["children"], json!([{"system": "sparks", "on": "spawn"}]));
    assert!(edit(&mut p, "child", json!({"child": {"system": "nope", "on": "spawn"}})).is_err());
    assert!(
        edit(&mut p, "child", json!({"child": {"system": "sparks", "on": "spawn", "probability": 3}}))
            .is_err()
    );
    edit(&mut p, "child", json!({"index": 0, "remove": true})).unwrap();
    assert!(system(&p).get("children").is_none());
    edit(&mut p, "over", json!({"size": [[0, 1], [1, 0]], "color": [[0, "#ffffff"], [1, "#ff000000"]]}))
        .unwrap();
    let r = &system(&p)["render"];
    assert_eq!(
        (&r["size_over_life"], &r["color_over_life"][1][1]),
        (&json!([[0.0, 1.0], [1.0, 0.0]]), &json!("#ff000000"))
    );
    edit(&mut p, "over", json!({"size": []})).unwrap();
    assert!(system(&p)["render"].get("size_over_life").is_none());
    let look = json!({"stretch": {"time": 0.1, "max": 40}, "frames": "random", "fps": 8, "mix": true, "brightness": 3});
    edit(&mut p, "render", look).unwrap();
    let r = &system(&p)["render"];
    assert_eq!((&r["frames"], &r["brightness"], &r["mix"]), (&json!("random"), &json!(3.0), &json!(true)));
    assert!(edit(&mut p, "render", json!({"brightness": 40})).is_err());
    assert!(edit(&mut p, "render", json!({"frames": "sometimes"})).is_err());
    edit(&mut p, "render", json!({"stretch": 0, "frames": "none", "mix": false})).unwrap();
    let r = &system(&p)["render"];
    assert!(r.get("stretch").is_none() && r.get("frames").is_none() && r.get("mix").is_none());
}

#[test]
fn a_starting_range_keeps_the_ends_and_the_curve_it_is_not_given() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let start = |ranges: serde_json::Value| json!({"ranges": ranges});
    edit(&mut p, "start", start(json!({"life": [1, {"prop": "density"}, 2]}))).unwrap();
    edit(&mut p, "start", start(json!({"life": [1.5, null, null]}))).unwrap();
    assert_eq!(system(&p)["init"]["life"], json!([1.5, {"prop": "density"}, 2]));
    edit(&mut p, "start", start(json!({"life": [1, 4, null]}))).unwrap();
    assert_eq!(system(&p)["init"]["life"], json!([1, 4, 2]));
    edit(&mut p, "start", start(json!({"life": [1, 4, 1]}))).unwrap();
    assert_eq!(system(&p)["init"]["life"], json!([1, 4]));
    assert!(edit(&mut p, "start", start(json!({"radial": [null, 4, null]}))).is_err());
}
