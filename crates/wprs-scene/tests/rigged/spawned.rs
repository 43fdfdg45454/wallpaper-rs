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
