use super::{act, ask, edit, layer, project, undone};
use serde_json::{Value, json};

fn animate(p: &mut wprs_editor::Project, args: Value) -> anyhow::Result<()> {
    act(p, json!({ "animate": args }))
}

fn named(p: &mut wprs_editor::Project, animation: &str) {
    let at = wprs_editor::actions::layer(&p.doc, "doll").unwrap().0;
    let pointer = format!("{at}/clips/0/animation");
    act(p, json!({ "set": { "pointer": pointer, "value": animation } })).unwrap();
}

pub fn rigged(root: &tempfile::TempDir) -> wprs_editor::Project {
    let mut p = project(root);
    std::fs::write(p.dir.join("rig.glb"), wprs_scene::ForgeRig::new(2.0, 0.5).glb()).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/assets/rig", "value": { "path": "rig.glb" } } }))
        .unwrap();
    edit(&mut p, json!({ "model": { "add": { "asset": "rig", "id": "doll" } } })).unwrap();
    act(&mut p, json!({ "clip": { "layer": "doll", "id": "bend", "add": true } })).unwrap();
    named(&mut p, "bend");
    p
}

#[test]
fn model_clips_take_3d_keys_and_bone_pickers_list_model_bones() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let before = p.doc.clone();
    let key = json!({ "layer": "doll", "clip": "bend", "time": 0.5, "bone": "tip", "angles": [0, 0, 30], "shift": [0, 0.1, 0] });
    act(&mut p, json!({ "key": key })).unwrap();
    undone(&mut p, &before);
    let clip = &layer(&p, "doll")["clips"][0]["bones"]["tip"];
    assert_eq!(clip["angles"], json!([[0.5, [0.0, 0.0, 30.0]]]));
    assert_eq!(clip["shift"][0][1], json!([0.0, 0.1, 0.0]));
    let bad = json!({ "layer": "doll", "clip": "bend", "time": 0.5, "bone": "nobody", "angles": [0, 0, 1] });
    assert!(act(&mut p, json!({ "key": bad })).is_err());
    let known = wprs_editor::rigs::known(&p.dir, &p.doc, "doll");
    let bones: Vec<&str> = known["bones"].as_array().unwrap().iter().filter_map(|b| b.as_str()).collect();
    assert!(bones.contains(&"tip"), "{known}");
    let m = ask(&p, json!({ "motion": { "layer": "doll" } }));
    assert_eq!(m["model"], true);
    assert_eq!(m["files"].as_array().unwrap().len(), 2);
    assert_eq!(m["shapes"], json!(["bulge"]));
    assert!(m["bones"].as_array().unwrap().iter().any(|b| b["name"] == "tip" && b["parent"] == "root"));
}

#[test]
fn animation_layers_are_added_tuned_masked_moved_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "clip": { "layer": "doll", "id": "walk", "add": true } })).unwrap();
    let before = p.doc.clone();
    animate(&mut p, json!({ "layer": { "layer": "doll", "add": "bend" } })).unwrap();
    undone(&mut p, &before);
    let tune = json!({ "layer": "doll", "index": 0, "weight": 0.5, "rate": 2, "additive": true, "blend_in": 0.3,
        "blend_out": 0.6, "enabled": false, "bones": ["tip"] });
    animate(&mut p, json!({ "layer": tune })).unwrap();
    let a = &layer(&p, "doll")["animations"][0];
    assert_eq!(
        (a["weight"].clone(), a["additive"].clone(), a["bones"].clone()),
        (json!(0.5), json!(true), json!(["tip"]))
    );
    animate(&mut p, json!({ "layer": { "layer": "doll", "add": "walk" } })).unwrap();
    animate(&mut p, json!({ "layer": { "layer": "doll", "index": 1, "to": 0 } })).unwrap();
    assert_eq!(layer(&p, "doll")["animations"][0]["clip"], "walk");
    for bad in [
        json!({ "layer": "doll", "index": 0, "weight": 2 }),
        json!({ "layer": "doll", "add": "nothing" }),
        json!({ "layer": "doll", "index": 9 }),
        json!({ "layer": "doll", "index": 0, "bones": ["nobody"] }),
        json!({ "layer": "doll", "index": 0, "rate": -1 }),
    ] {
        assert!(animate(&mut p, json!({ "layer": bad.clone() })).is_err(), "{bad}");
    }
    let m = ask(&p, json!({ "motion": { "layer": "doll" } }));
    assert_eq!(m["layers"].as_array().unwrap().len(), 2);
    animate(&mut p, json!({ "layer": { "layer": "doll", "index": 0, "remove": true } })).unwrap();
    animate(&mut p, json!({ "layer": { "layer": "doll", "index": 0, "remove": true } })).unwrap();
    assert!(layer(&p, "doll").get("animations").is_none());
}

#[test]
fn root_motion_presets_and_blend_shapes_of_a_model() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    named(&mut p, "walk");
    let before = p.doc.clone();
    animate(&mut p, json!({ "root": { "layer": "doll", "on": true } })).unwrap();
    undone(&mut p, &before);
    animate(&mut p, json!({ "layer": { "layer": "doll", "add": "bend" } })).unwrap();
    let m = ask(&p, json!({ "motion": { "layer": "doll" } }));
    let path = m["clips"][0]["path"].as_array().unwrap();
    assert!((path.last().unwrap()[0].as_f64().unwrap() - 2.0).abs() < 0.05, "two laps of the walk: {path:?}");
    let shown = ask(&p, json!({ "stage3d": { "time": 0.5 } }));
    let doll = &shown["models"][0];
    assert!(doll["bones"].as_array().is_some_and(|b| b.len() >= 2), "{doll}");
    assert!(doll["path"].as_array().is_some_and(|p| p.len() > 10), "{doll}");
    animate(&mut p, json!({ "preset": { "layer": "doll", "bone": "tip", "preset": "hair" } })).unwrap();
    assert_eq!(layer(&p, "doll")["physics"][0]["mode"], "rope");
    assert!(
        animate(&mut p, json!({ "preset": { "layer": "doll", "bone": "tip", "preset": "nothing" } }))
            .is_err()
    );
    animate(&mut p, json!({ "morph": { "layer": "doll", "shape": "bulge", "weight": 0.7 } })).unwrap();
    assert_eq!(layer(&p, "doll")["shapes"]["bulge"], json!(0.7));
    assert!(animate(&mut p, json!({ "morph": { "layer": "doll", "shape": "bulge", "weight": 3 } })).is_err());
    animate(&mut p, json!({ "morph": { "layer": "doll", "shape": "bulge", "weight": null } })).unwrap();
    assert!(layer(&p, "doll").get("shapes").is_none());
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "plain" } } })).unwrap();
    assert!(animate(&mut p, json!({ "root": { "layer": "plain", "on": true } })).is_err(), "needs clips");
}
