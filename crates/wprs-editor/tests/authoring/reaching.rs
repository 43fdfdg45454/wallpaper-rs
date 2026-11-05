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
