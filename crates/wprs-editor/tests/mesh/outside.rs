use super::kit::{act, project, rigged};
use serde_json::json;

fn templated(p: &mut wprs_editor::Project, id: &str) -> String {
    act(p, json!({ "duplicate_layer": { "id": id, "template": true } })).unwrap();
    let name = format!("{id}_template");
    p.scope = Some(name.clone());
    name
}

#[test]
fn layers_inside_a_template_follow_reach_for_and_mask_by_layers_of_the_scene() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "badge" } })).unwrap();
    let badge = templated(&mut p, "badge");
    act(&mut p, json!({ "attach": { "layer": "badge", "to": "net", "bone": "arm" } })).unwrap();
    let copied = &p.doc["scene"]["templates"][&badge];
    assert_eq!(copied["attach"], json!({ "layer": "net", "bone": "arm" }), "{copied}");
    let outline = wprs_editor::outline::outline(&p, 0.0, &[]);
    assert!(outline["outside"].as_array().unwrap().iter().any(|l| l["id"] == "net" && l["type"] == "mesh"));
    assert_eq!(
        wprs_editor::rigs::query(&p.dir, &p.holding("net"), "net", &json!({}))["bones"][1]["name"],
        "arm"
    );
    act(&mut p, json!({ "attach": { "layer": "badge", "to": "net", "point": [0.25, 0.5] } })).unwrap();
    let point = p.doc["scene"]["templates"][&badge]["attach"]["point"].clone();
    assert_eq!(point, json!("badge_point"));
    let named = |p: &wprs_editor::Project| {
        let info = wprs_editor::rigs::rig(&p.dir, &p.doc, "net");
        info["points"].as_array().unwrap().iter().any(|q| q["name"] == point)
    };
    assert!(named(&p), "the point is placed on the mesh of the scene");
    act(&mut p, json!({ "attach": { "layer": "badge", "detach": true } })).unwrap();
    assert!(!named(&p), "a point nothing follows goes with the attachment");
    act(&mut p, json!({ "effects": { "add": { "layer": "badge", "effect": "vignette" } } })).unwrap();
    act(&mut p, json!({ "effects": { "mask": { "layer": "badge", "index": 0, "from": "net" } } })).unwrap();
    assert_eq!(p.doc["scene"]["templates"][&badge]["effects"][0]["mask"]["layer"], "net");
    p.scope = None;
    templated(&mut p, "net");
    act(&mut p, json!({ "ik": { "layer": "net", "root": "root", "tip": "arm", "target": "layer:badge" } }))
        .unwrap();
    assert_eq!(p.doc["scene"]["templates"]["net_template"]["ik"][0]["target"], "layer:badge");
    let missing = act(&mut p, json!({ "ik": { "layer": "net", "index": 0, "target": "layer:nothing" } }));
    assert!(missing.is_err());
    let problems = wprs_editor::check::review(&p.doc, Some(&p.dir));
    assert!(problems.iter().all(|q| q.warning), "{problems:?}");
}

#[test]
fn property_edits_while_a_template_is_shown_keep_the_bindings_of_the_scene() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    act(&mut p, json!({ "add_property": { "kind": "slider", "id": "amount" } })).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "fill" } })).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "other" } })).unwrap();
    let other = wprs_editor::layers::find(&p.doc, "other").unwrap();
    act(&mut p, json!({ "set": { "pointer": format!("{other}/opacity"), "value": { "prop": "amount" } } }))
        .unwrap();
    templated(&mut p, "fill");
    let refused = act(&mut p, json!({ "remove_property": { "id": "amount" } })).unwrap_err().to_string();
    assert!(refused.contains("opacity"), "{refused}");
    assert!(p.doc["manifest"]["properties"].get("amount").is_some());
    act(&mut p, json!({ "rename_property": { "id": "amount", "to": "level" } })).unwrap();
    assert_eq!(p.doc.pointer(&format!("{other}/opacity")), Some(&json!({ "prop": "level" })));
    let problems = wprs_editor::check::review(&p.doc, Some(&p.dir));
    assert!(problems.iter().all(|q| q.warning), "{problems:?}");
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc.pointer(&format!("{other}/opacity")), Some(&json!({ "prop": "amount" })));
}
