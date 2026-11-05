use super::{act, at, project, refused};
use serde_json::json;

fn key(extra: serde_json::Value) -> serde_json::Value {
    let mut k = json!({"pointer": "/scene/layers/0/opacity"});
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "keyframe": k })
}

#[test]
fn timelines_are_added_timed_renamed_and_removed_with_the_values_they_drive() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"timeline": {"id": "intro", "add": true, "loop": "repeat"}})).unwrap();
    assert_eq!(at(&p, "/scene/timelines"), json!([{"id": "intro", "duration": 5, "loop": "repeat"}]));
    act(&mut p, key(json!({"time": 0, "value": 0, "timeline": "intro"}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity/timeline"), "intro");
    act(&mut p, json!({"timeline": {"id": "intro", "frames": 48, "fps": 24, "wrap": true}})).unwrap();
    let line = at(&p, "/scene/timelines/0");
    assert_eq!((&line["frames"], &line["fps"], line.get("duration")), (&json!(48), &json!(24.0), None));
    act(&mut p, json!({"timeline": {"id": "intro", "to": "opening", "speed": -2, "delay": 1.5}})).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity/timeline"), "opening");
    assert_eq!(at(&p, "/scene/timelines/0/speed"), json!(-2.0));
    for bad in [
        json!({"timeline": {"id": "opening", "add": true}}),
        json!({"timeline": {"id": "Big one", "add": true}}),
        json!({"timeline": {"id": "nowhere", "duration": 2}}),
        json!({"timeline": {"id": "opening", "fps": 500}}),
        json!({"timeline": {"id": "opening", "duration": 2, "frames": 10}}),
        json!({"timeline": {"id": "opening", "speed": 900}}),
        json!({"timeline": {"id": "opening", "loop": "once"}}),
        json!({"timeline": {"id": "opening", "trigger": "never_heard_of"}}),
    ] {
        refused(&mut p, bad);
    }
    act(&mut p, json!({"timeline": {"id": "opening", "remove": true}})).unwrap();
    assert_eq!(at(&p, "/scene/timelines"), json!([]));
    assert!(at(&p, "/scene/layers/0/opacity").get("timeline").is_none());
    let missing = refused(&mut p, key(json!({"time": 1, "value": 1, "timeline": "opening"})));
    assert!(missing.contains("timeline action"), "{missing}");
}

#[test]
fn keyframes_take_a_base_and_bezier_handles_that_the_editor_samples() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, key(json!({"time": 0, "value": 0}))).unwrap();
    act(&mut p, key(json!({"time": 2, "value": 1, "base": 0.25}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity/base"), json!(0.25));
    act(&mut p, key(json!({"time": 0, "out_handle": [1.0, 0.0]}))).unwrap();
    act(&mut p, key(json!({"time": 2, "in_handle": [1.0, 0.0]}))).unwrap();
    let keys = at(&p, "/scene/layers/0/opacity/keys");
    assert_eq!(keys, json!([[0.0, 0, {"out": [1.0, 0.0]}], [2.0, 1, {"in": [1.0, 0.0]}]]));
    let keys = keys.as_array().unwrap();
    let early = wprs_editor::keys::sample(keys, 0.4).unwrap().as_f64().unwrap();
    assert!(early < 0.2 * 0.5, "a flat handle starts slowly: {early}");
    assert_eq!(wprs_editor::keys::sample(keys, 1.0), Some(json!(0.5)));
    act(&mut p, key(json!({"time": 0, "ease": "in_out_sine"}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity/keys/0/2"), "in_out_sine");
    act(&mut p, key(json!({"time": 0, "absolute": true}))).unwrap();
    assert!(at(&p, "/scene/layers/0/opacity").get("base").is_none());
    for bad in [
        key(json!({"time": 0, "ease": "linear", "out_handle": [1, 0]})),
        key(json!({"time": 0, "out_handle": [-1, 0]})),
        key(json!({"time": 0, "base": "#ffffff"})),
        key(json!({"time": 0, "base": 1, "absolute": true})),
    ] {
        refused(&mut p, bad);
    }
    act(&mut p, json!({"keyframe": {"pointer": "/scene/layers/0/color", "time": 0, "value": "#102030"}}))
        .unwrap();
    let words = refused(
        &mut p,
        json!({"keyframe": {"pointer": "/scene/layers/0/color", "time": 0, "in_handle": [1, 0]}}),
    );
    assert!(words.contains("single numbers"), "{words}");
}

#[test]
fn a_timeline_that_drives_a_tracked_binding_is_renamed_and_removed_with_it() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({"timeline": {"id": "intro", "add": true}})).unwrap();
    let tracked = json!({"x": {"keys": [[0, 0], [1, 100]]}, "y": 5, "timeline": "intro"});
    let at_position = "/scene/layers/0/transform/position";
    act(&mut p, json!({"set": {"pointer": at_position, "value": tracked}})).unwrap();
    act(&mut p, json!({"timeline": {"id": "intro", "to": "main"}})).unwrap();
    assert_eq!(at(&p, &format!("{at_position}/timeline")), "main");
    act(&mut p, json!({"timeline": {"id": "main", "remove": true}})).unwrap();
    assert!(at(&p, at_position).get("timeline").is_none());
    let problems = wprs_editor::check::review(&p.doc, Some(&p.dir));
    assert!(problems.iter().all(|p| p.warning), "{problems:?}");
}

#[test]
fn removing_the_last_relative_keyframe_keeps_the_value_it_showed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let at_position = "/scene/layers/0/transform/position";
    let keyed = |extra: serde_json::Value| {
        let mut k = json!({"pointer": at_position, "time": 0});
        k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({ "keyframe": k })
    };
    act(&mut p, keyed(json!({"value": [10, 0], "base": [960, 540]}))).unwrap();
    act(&mut p, keyed(json!({"remove": true}))).unwrap();
    assert_eq!(at(&p, at_position), json!([970.0, 540.0]));
    act(&mut p, key(json!({"time": 0, "value": 0.5, "base": 0.25}))).unwrap();
    act(&mut p, key(json!({"time": 0, "remove": true, "absolute": true}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity"), json!(0.5));
    act(&mut p, json!({"keyframe": {"pointer": "/scene/layers/0/color", "time": 0, "value": "#102030"}}))
        .unwrap();
    let color = |extra: serde_json::Value| {
        let mut k = json!({"pointer": "/scene/layers/0/color", "time": 0});
        k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({ "keyframe": k })
    };
    act(&mut p, color(json!({"base": "#010101"}))).unwrap();
    act(&mut p, color(json!({"remove": true}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/color"), "#112131");
}

#[test]
fn straightening_a_curve_is_one_step_that_keeps_the_neighbouring_curves() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let bent = json!({"in": [0.3, 0.1], "out": [0.3, 0.1]});
    let keys = json!({"keys": [[0, 0, {"out": [0.3, 0.1]}], [1, 1, bent], [2, 0, bent], [3, 1, {"in": [0.3, 0.1]}]]});
    act(&mut p, json!({"set": {"pointer": "/scene/layers/0/opacity", "value": keys}})).unwrap();
    let before = p.history.done.len();
    act(&mut p, key(json!({"time": 1, "straighten": true}))).unwrap();
    assert_eq!(p.history.done.len(), before + 1);
    let got = at(&p, "/scene/layers/0/opacity/keys");
    assert_eq!(got[1], json!([1, 1, {"in": [0.3, 0.1]}]));
    assert_eq!(got[2], json!([2, 0, {"out": [0.3, 0.1]}]));
    assert_eq!((&got[0], &got[3]), (&keys["keys"][0], &keys["keys"][3]));
    act(&mut p, key(json!({"time": 0, "ease": "in_out_sine"}))).unwrap();
    act(&mut p, key(json!({"time": 0, "straighten": true}))).unwrap();
    assert_eq!(at(&p, "/scene/layers/0/opacity/keys/0"), json!([0.0, 0]));
    refused(&mut p, key(json!({"time": 1, "straighten": true, "ease": "linear"})));
    refused(&mut p, key(json!({"time": 5, "straighten": true})));
}
