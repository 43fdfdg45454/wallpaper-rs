use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::Scene;

mod particles;
mod scripts;

const DEEP: &str = include_str!("../deep.json");

fn faults(changes: &[(&str, Json)]) -> Vec<String> {
    let mut doc: Json = serde_json::from_str(DEEP).unwrap();
    for (path, value) in changes {
        let (parent, key) = path.rsplit_once('/').unwrap();
        match doc.pointer_mut(path) {
            Some(slot) => *slot = value.clone(),
            None => doc.pointer_mut(parent).unwrap_or_else(|| panic!("{path}"))[key] = value.clone(),
        }
    }
    match Scene::from_json(&doc.to_string()) {
        Ok(scene) => scene.validate(&BTreeMap::new()).iter().map(ToString::to_string).collect(),
        Err(e) => vec![e.to_string()],
    }
}

fn expect(changes: &[(&str, Json)], fault: &str) {
    let found = faults(changes);
    assert!(found.iter().any(|f| f.contains(fault)), "{changes:?}: expected {fault:?}, found {found:?}");
}

#[test]
fn the_fixture_is_valid() {
    assert_eq!(faults(&[]), Vec::<String>::new());
}

#[test]
fn meshes_models_and_cameras_name_their_broken_fields() {
    let m = "/layers/2/layers/0";
    for (changes, fault) in [
        (vec![("/layers/0/mesh", json!("skin"))], "/layers/0/mesh does not fit"),
        (vec![("/layers/0/mesh", json!("ghost"))], "/layers/0/mesh names the unknown asset \"ghost\""),
        (vec![("/layers/0/clips/1/id", json!("idle"))], "/layers/0/clips/1/id repeats an id"),
        (vec![("/layers/0/animations/0/clip", json!("run"))], "names the unknown clip \"run\""),
        (
            vec![("/layers/0/animations/0/weight", json!(2))],
            "/layers/0/animations/0/weight is 2, expected 0..=1",
        ),
        (vec![("/layers/0/ik/0/chain", json!(["arm"]))], "/layers/0/ik/0/chain is 1, expected 2..=8"),
        (vec![("/layers/0/ik/0/chain", json!(["arm", "arm"]))], "/layers/0/ik/0/chain repeats an id"),
        (
            vec![("/layers/0/ik/0/target", json!("layer:fox"))],
            "/layers/0/ik/0/target names the unknown layer",
        ),
        (vec![("/layers/0/ik/0/target", json!("nose"))], "names the unknown target \"nose\""),
        (vec![("/layers/0/ik/0/limits", json!({ "leg": [0, 1] }))], "unknown bone of the chain \"leg\""),
        (vec![("/layers/0/ik/0/limits/arm", json!([90, -90]))], "/layers/0/ik/0/limits/arm/1 is -90"),
        (
            vec![("/layers/0/physics/0/stiffness", json!(2))],
            "/layers/0/physics/0/stiffness is 2, expected 0..=1",
        ),
        (vec![("/layers/0/physics/0/mode", json!("jelly"))], "unknown variant `jelly`"),
        (vec![("/layers/0/rules/0/parent", json!("sword"))], "/layers/0/rules/0/parent depends on itself"),
        (vec![("/layers/0/depth/scale", json!(20))], "/layers/0/depth/scale is 20, expected 0..=10"),
        (vec![("/layers/0/expressions/smile/cheeks", json!(3))], "/layers/0/expressions/smile/cheeks is 3"),
        (vec![("/layers/0/shapes/smile", json!(2))], "/layers/0/shapes/smile is 2, expected 0..=1"),
        (
            vec![("/layers/0/parts/arm/clip_to", json!("arm"))],
            "/layers/0/parts/arm/clip_to names the unknown part",
        ),
        (vec![("/layers/1/attach/layer", json!("badge"))], "names the unknown mesh or model layer \"badge\""),
        (vec![("/layers/1/attach/point", json!("socket"))], "/layers/1/attach does not fit"),
        (vec![("/layers/1/parent", json!("fox"))], "/layers/1/attach does not fit"),
        (
            vec![("/layers/3/depth_test", json!(true))],
            "/layers/3/depth_test applies inside a group with space 3d",
        ),
        (vec![(&format!("{m}/materials/metal/params/metallic"), json!(3))], "metallic is 3, expected 0..=1"),
        (
            vec![(&format!("{m}/materials/metal/params/shine"), json!(1))],
            "names the unknown parameter \"shine\"",
        ),
        (vec![(&format!("{m}/materials/metal/shader"), json!("lava"))], "names the unknown shader \"lava\""),
        (
            vec![(&format!("{m}/clips"), json!([])), (&format!("{m}/animations"), json!([]))],
            "root_motion does not fit",
        ),
        (vec![("/layers/2/layers/1/light", json!("tube"))], "/layers/2/layers/1/shadows does not fit"),
        (vec![("/camera3d/fov", json!(200))], "/camera3d/fov is 200, expected 1..=179"),
        (vec![("/camera3d/far", json!(0.01))], "/camera3d/far is 0.01"),
        (vec![("/camera3d/up", json!([0, 0, 0]))], "/camera3d/up does not fit"),
        (
            vec![("/camera3d/paths/0/keys", json!([{ "t": 0, "eye": 1, "center": 0 }]))],
            "/camera3d/paths/0/keys is 1, expected 2..=256",
        ),
        (vec![("/camera3d/paths/0/keys/1/fov", json!(0))], "/camera3d/paths/0/keys/1/fov is 0"),
    ] {
        expect(&changes, fault);
    }
}
