use super::kit::{arm, fox};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn templated(template: serde_json::Value) -> Result<Runtime, SceneError> {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 64] }, "assets": { "arm": { "path": "arm.glb" } },
        "layers": [], "templates": { "fox": template } });
    let mut scene = Scene::from_json(&text.to_string())?;
    let glb = arm().bytes();
    scene.adopt(&BTreeMap::new(), move |_: &str, _: usize| Ok::<_, String>(glb.clone()))?;
    Ok(Runtime::new(Arc::new(scene), Values::new()))
}

fn spawn(rt: &mut Runtime, ids: &[&str]) -> bool {
    let spawned = ids.iter().map(|id| (id.to_string(), "fox".to_string())).collect();
    rt.set_script(ScriptOutputs { spawned, ..ScriptOutputs::default() })
}

fn meshes(rt: &Runtime) -> Vec<String> {
    let frame = rt.frame([0.0, 0.0, 64.0, 64.0], Quality::Q3);
    let drawn = frame.draws.iter().filter_map(|d| match &d.content {
        Content::Mesh { layer, .. } => Some(layer.to_string()),
        _ => None,
    });
    drawn.collect()
}

#[test]
fn spawned_mesh_templates_get_a_puppet_and_draw() {
    let mut rt = templated(fox(json!({ "id": "body" }))).unwrap();
    assert!(meshes(&rt).is_empty());
    assert!(spawn(&mut rt, &["a", "b"]));
    assert!(rt.puppet("a").is_some() && rt.puppet("b").is_some());
    assert_eq!(meshes(&rt), ["a", "b"]);
    assert!(spawn(&mut rt, &["b"]));
    assert!(rt.puppet("a").is_none() && rt.puppet("b").is_some());
    assert_eq!(meshes(&rt), ["b"]);
}

#[test]
fn mesh_children_of_group_templates_draw_too() {
    let group = json!({ "id": "pack", "type": "group", "layers": [fox(json!({ "id": "body" }))] });
    let mut rt = templated(group).unwrap();
    assert!(spawn(&mut rt, &["one"]));
    assert!(rt.puppet("one.body").is_some());
    assert_eq!(meshes(&rt), ["one.body"]);
    assert!(spawn(&mut rt, &[]));
    assert!(rt.puppet("one.body").is_none());
}

#[test]
fn templates_still_refuse_layers_scripts_cannot_copy() {
    let sound = json!({ "id": "s", "type": "sound", "sound": "arm" });
    let refused = templated(sound).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(refused.contains("/templates/fox/type"), "{refused}");
    assert!(templated(fox(json!({ "id": "body" }))).is_ok());
}

fn pack() -> serde_json::Value {
    let waving = json!({ "id": "body", "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }],
        "animations": [{ "clip": "wave" }], "ik": [{ "chain": ["arm", "hand"], "target": "layer:badge", "weight": 0 }] });
    let badge = json!({ "id": "badge", "type": "solid", "color": "#ffffff", "size": [4, 4],
        "attach": { "layer": "body", "bone": "hand" } });
    json!({ "id": "pack", "type": "group", "layers": [fox(waving), badge] })
}

#[test]
fn attachments_and_chains_inside_a_template_follow_their_own_copy() {
    let mut rt = templated(pack()).unwrap();
    let placed = |at: [f32; 2]| LayerOverride { position: Some(at), ..LayerOverride::default() };
    let spawned = vec![("a".to_string(), "fox".to_string()), ("b".into(), "fox".into())];
    let layers = [("a".to_string(), placed([0.0, 0.0])), ("b".into(), placed([20.0, 10.0]))].into();
    rt.set_script(ScriptOutputs { spawned, layers, ..ScriptOutputs::default() });
    rt.advance(2.0);
    let frame = rt.frame([0.0, 0.0, 64.0, 64.0], Quality::Q3);
    let at = |id: &str| {
        let w = frame.draws.iter().find(|d| d.layer.id == id).unwrap().world.0;
        [w[4], w[5]]
    };
    let (a, b) = (at("a.badge"), at("b.badge"));
    assert!((a[0] - 32.0).abs() < 1e-3 && (a[1] - 8.0).abs() < 1e-3, "{a:?}");
    assert!((b[0] - a[0] - 20.0).abs() < 1e-3 && (b[1] - a[1] - 10.0).abs() < 1e-3, "{a:?} {b:?}");
    let target = |id: &str| match &rt.layer(id).unwrap().kind {
        Kind::Mesh(m) => m.rig.ik[0].target.clone(),
        _ => unreachable!(),
    };
    assert_eq!(target("b.body"), Reach::Named("layer:b.badge".into()));
}

#[test]
fn attachments_inside_templates_name_layers_of_the_template_or_the_scene() {
    let mut lost = pack();
    lost["layers"][1]["attach"]["layer"] = json!("nobody");
    let refused = templated(lost).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(refused.contains("/templates/fox/layers/1/attach/layer"), "{refused}");
    let mut astray = pack();
    astray["layers"][0]["ik"][0]["target"] = json!("layer:nobody");
    let refused = templated(astray).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(refused.contains("/templates/fox/layers/0/ik/0/target"), "{refused}");
}
