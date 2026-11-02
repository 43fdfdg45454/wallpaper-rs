use super::{act, edit, layer, project, undone};
use serde_json::json;

#[test]
fn models_are_added_placed_and_removed_with_undo() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll", "at": [1, 0, -2] } } })).unwrap();
    undone(&mut p, &before);
    assert_eq!(layer(&p, "doll")["type"], "model");
    assert_eq!(layer(&p, "doll")["position"], json!([1.0, 0.0, -2.0]));
    let before = p.doc.clone();
    let set = json!({ "layer": "doll", "position": [0, 1, 0], "rotation": [0, 45, 0], "scale": [2, 2, 2], "env": "sky" });
    edit(&mut p, json!({ "model": { "set": set } })).unwrap();
    undone(&mut p, &before);
    let toy = layer(&p, "doll");
    assert_eq!((toy["rotation"].clone(), toy["env"].clone()), (json!([0.0, 45.0, 0.0]), json!("sky")));
    for bad in [
        json!({ "set": { "layer": "doll", "scale": [0, 1, 1] } }),
        json!({ "set": { "layer": "doll", "position": [1e9, 0, 0] } }),
        json!({ "set": { "layer": "doll", "env": "toy" } }),
        json!({ "set": { "layer": "nothing", "position": [0, 0, 0] } }),
        json!({ "add": { "asset": "sky" } }),
    ] {
        assert!(edit(&mut p, json!({ "model": bad.clone() })).is_err(), "{bad}");
    }
    act(&mut p, json!({ "add_layer": { "kind": "model", "id": "menu" } })).unwrap();
    assert_eq!(layer(&p, "menu")["model"], "toy");
    edit(&mut p, json!({ "model": { "remove": { "layer": "menu" } } })).unwrap();
    assert!(wprs_editor::actions::layer(&p.doc, "menu").is_err());
}

#[test]
fn groups_turn_3d_and_their_flat_layers_choose_depth() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "solid", "id": "card" } })).unwrap();
    assert!(edit(&mut p, json!({ "depth_test": { "layer": "card", "on": true } })).is_err());
    let before = p.doc.clone();
    edit(&mut p, json!({ "group": { "id": "world", "layers": ["doll", "card"] } })).unwrap();
    undone(&mut p, &before);
    assert_eq!(layer(&p, "world")["space"], "3d");
    let before = p.doc.clone();
    edit(&mut p, json!({ "depth_test": { "layer": "card", "on": true } })).unwrap();
    undone(&mut p, &before);
    assert_eq!(layer(&p, "card")["depth_test"], true);
    assert!(edit(&mut p, json!({ "depth_test": { "layer": "doll", "on": true } })).is_err());
    edit(&mut p, json!({ "space": { "layer": "world", "deep": false } })).unwrap();
    assert!(layer(&p, "world").get("space").is_none());
    assert!(edit(&mut p, json!({ "space": { "layer": "card", "deep": true } })).is_err());
    edit(&mut p, json!({ "group": { "id": "empty" } })).unwrap();
    assert_eq!(
        (layer(&p, "empty")["space"].clone(), layer(&p, "empty")["layers"].clone()),
        (json!("3d"), json!([]))
    );
}

#[test]
fn materials_change_by_name_and_check_their_values() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let before = p.doc.clone();
    let m = json!({ "layer": "doll", "name": "paint", "shader": "unlit", "double_sided": true, "cast_shadows": true,
        "emissive": "#ff0000" });
    edit(&mut p, json!({ "material": m })).unwrap();
    undone(&mut p, &before);
    let paint = &layer(&p, "doll")["materials"]["paint"];
    assert_eq!((paint["shader"].clone(), paint["double_sided"].clone()), (json!("unlit"), json!(true)));
    edit(&mut p, json!({ "material": { "layer": "doll", "name": "trim", "params": { "metallic": 0.8, "roughness": 0.2 } } })).unwrap();
    assert_eq!(layer(&p, "doll")["materials"]["trim"]["params"]["metallic"], json!(0.8));
    for bad in [
        json!({ "layer": "doll", "name": "ghost", "lit": false }),
        json!({ "layer": "doll", "name": "trim", "params": { "metallic": 2 } }),
        json!({ "layer": "doll", "name": "trim", "params": { "wobble": 1 } }),
        json!({ "layer": "doll", "name": "trim", "shader": "plastic" }),
        json!({ "layer": "doll", "name": "trim", "color": "blue" }),
    ] {
        assert!(edit(&mut p, json!({ "material": bad.clone() })).is_err(), "{bad}");
    }
    edit(&mut p, json!({ "material": { "layer": "doll", "name": "paint", "clear": true } })).unwrap();
    assert!(layer(&p, "doll")["materials"].get("paint").is_none());
}
