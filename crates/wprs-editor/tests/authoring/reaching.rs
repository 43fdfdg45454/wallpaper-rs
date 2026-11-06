use super::{act, at, project, refused};
use serde_json::json;
use wprs_editor::Project;

fn key(extra: serde_json::Value) -> serde_json::Value {
    let mut k = json!({"pointer": "/scene/layers/0/opacity"});
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "keyframe": k })
}

fn sound(p: &Project) {
    let problems = wprs_editor::check::review(&p.doc, Some(&p.dir));
    let errors: Vec<_> = problems.iter().filter(|p| !p.warning).map(|p| &p.message).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn bezier_handles_reach_only_up_to_the_neighbouring_keyframe_so_the_scene_stays_valid() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, key(json!({"time": 0, "value": 1}))).unwrap();
    let lone = refused(&mut p, key(json!({"time": 0, "out_handle": [1, 0]})));
    assert!(lone.contains("neighbouring keyframe"), "{lone}");
    act(&mut p, key(json!({"time": 3, "value": 0.2}))).unwrap();
    refused(&mut p, key(json!({"time": 0, "out_handle": [5, 0]})));
    refused(&mut p, key(json!({"time": 3, "out_handle": [1, 0]})));
    refused(&mut p, key(json!({"time": 0, "in_handle": [0.5, 0]})));
    act(&mut p, key(json!({"time": 0, "out_handle": [3, 0]}))).unwrap();
    act(&mut p, key(json!({"time": 3, "in_handle": [2, 0.1]}))).unwrap();
    sound(&p);
    refused(&mut p, key(json!({"time": 1, "value": 0.5})));
    refused(&mut p, key(json!({"time": 2, "from": 3})));
    refused(&mut p, key(json!({"time": 3, "remove": true})));
    act(&mut p, key(json!({"time": 0, "out_handle": [1, 0]}))).unwrap();
    act(&mut p, key(json!({"time": 3, "in_handle": [1, 0]}))).unwrap();
    act(&mut p, key(json!({"time": 1.5, "value": 0.5}))).unwrap();
    act(&mut p, key(json!({"time": 5, "from": 3}))).unwrap();
    sound(&p);
    assert_eq!(at(&p, "/scene/layers/0/opacity/keys/2/2"), json!({"in": [1.0, 0.0]}));
}

#[test]
fn handle_values_bases_and_keyframe_values_stay_within_what_the_scene_accepts() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, key(json!({"time": 0, "value": 1}))).unwrap();
    act(&mut p, key(json!({"time": 3, "value": 0.2}))).unwrap();
    let out = refused(&mut p, key(json!({"time": 0, "out_handle": [1, 2_000_000]})));
    assert!(out.contains("1000000"), "{out}");
    refused(&mut p, key(json!({"time": 3, "in_handle": [1, -2_000_000]})));
    let base = refused(&mut p, key(json!({"time": 0, "base": 5_000_000})));
    assert!(base.contains("1000000"), "{base}");
    refused(&mut p, key(json!({"time": 0, "base": -1_000_001})));
    refused(&mut p, key(json!({"time": 0, "value": 5_000_000})));
    act(&mut p, key(json!({"time": 0, "out_handle": [1, 1_000_000]}))).unwrap();
    act(&mut p, key(json!({"time": 3, "base": -1_000_000}))).unwrap();
    sound(&p);
}

#[test]
fn list_bases_keep_every_number_within_what_the_scene_accepts() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let scale = |extra: serde_json::Value| {
        let mut k = json!({"pointer": "/scene/layers/0/transform/scale"});
        k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({ "keyframe": k })
    };
    act(&mut p, scale(json!({"time": 0, "value": [1, 1]}))).unwrap();
    let error = refused(&mut p, scale(json!({"time": 0, "base": [1, 3_000_000]})));
    assert!(error.contains("1000000"), "{error}");
    act(&mut p, scale(json!({"time": 0, "base": [0.5, 2]}))).unwrap();
    sound(&p);
}

#[test]
fn a_keyframe_value_the_scene_cannot_read_is_refused_with_the_shape_it_needs() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let at_scale = |extra: serde_json::Value| {
        let mut k = json!({"pointer": "/scene/layers/0/transform/scale", "time": 0});
        k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({ "keyframe": k })
    };
    act(&mut p, at_scale(json!({"value": [1, 1]}))).unwrap();
    let listed = refused(&mut p, at_scale(json!({"time": 1, "value": "1,1"})));
    assert!(
        listed.contains("a list of 2 numbers such as [1, 1]") && !listed.contains("untagged"),
        "{listed}"
    );
    act(&mut p, at_scale(json!({"time": 1, "value": 2}))).unwrap();
    act(&mut p, key(json!({"time": 0, "value": 0.5}))).unwrap();
    let number = refused(&mut p, key(json!({"time": 1, "value": "half"})));
    assert!(number.contains("give a number such as 0.5") && number.contains("\"half\""), "{number}");
    act(&mut p, json!({"keyframe": {"pointer": "/scene/layers/0/color", "time": 0, "value": "#102030"}}))
        .unwrap();
    let color =
        refused(&mut p, json!({"keyframe": {"pointer": "/scene/layers/0/color", "time": 1, "value": 3}}));
    assert!(color.contains("a color such as"), "{color}");
    sound(&p);
}
