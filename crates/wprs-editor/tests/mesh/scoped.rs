use super::kit::{act, rigged};
use serde_json::json;

#[test]
fn layers_inside_a_template_get_bones_chains_and_attachments_of_their_own() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "badge" } })).unwrap();
    act(&mut p, json!({ "group": { "ids": ["net", "badge"], "id": "fox" } })).unwrap();
    act(&mut p, json!({ "duplicate_layer": { "id": "fox", "template": true } })).unwrap();
    p.scope = Some("fox_template".into());
    act(&mut p, json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.9, 0.5] } }))
        .unwrap();
    act(&mut p, json!({ "ik": { "layer": "net", "root": "root", "tip": "hand" } })).unwrap();
    act(&mut p, json!({ "attach": { "layer": "badge", "to": "net", "bone": "arm" } })).unwrap();
    let template = &p.doc["scene"]["templates"]["fox_template"];
    let net = template["layers"].as_array().unwrap().iter().find(|l| l["id"] == "net").unwrap();
    let badge = template["layers"].as_array().unwrap().iter().find(|l| l["id"] == "badge").unwrap();
    assert_eq!(net["ik"][0]["chain"], json!(["root", "arm", "hand"]), "{template}");
    assert_eq!(badge["attach"], json!({ "layer": "net", "bone": "arm" }));
    let scene = &p.doc["scene"]["layers"][0]["layers"];
    assert!(scene.as_array().unwrap().iter().all(|l| l.get("ik").is_none() && l.get("attach").is_none()));
    let seen = p.seen();
    let shown = wprs_editor::rigs::query(&p.dir, &seen, "net", &json!({ "handles": true }));
    assert_eq!(shown["ik"].as_array().map(Vec::len), Some(1), "{shown}");
    let outline = wprs_editor::outline::outline(&p, 0.0, &[]);
    let pointers: Vec<&str> =
        outline["layers"].as_array().unwrap().iter().filter_map(|l| l["pointer"].as_str()).collect();
    assert_eq!(pointers[0], "/scene/templates/fox_template", "{pointers:?}");
    assert!(
        outline["gizmos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["pointer"].as_str().unwrap().starts_with("/scene/templates/"))
    );
    let one = act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "loose" } }));
    assert!(one.unwrap_err().to_string().contains("one layer"), "a template has one root layer");
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "spot", "into": "fox" } })).unwrap();
    assert!(
        p.doc["scene"]["templates"]["fox_template"]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["id"] == "spot")
    );
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert!(
        !p.doc["scene"]["templates"]["fox_template"]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["id"] == "spot")
    );
    act(&mut p, json!({ "group": { "ids": ["fox"], "id": "box" } })).unwrap();
    let boxed = &p.doc["scene"]["templates"]["fox_template"];
    assert_eq!(
        (&boxed["id"], &boxed["layers"][0]["id"]),
        (&json!("box"), &json!("fox")),
        "the root became a group"
    );
    p.scope = Some("missing".into());
    assert!(act(&mut p, json!({ "hide": { "id": "net", "hidden": true } })).is_err(), "no such template");
}
