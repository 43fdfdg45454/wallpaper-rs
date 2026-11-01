use super::kit::{act, info, parts};
use serde_json::{Value, json};
use wprs_editor::Project;

fn spots(p: &Project) -> Vec<(Value, Value)> {
    let list = wprs_editor::rigs::vertices(&p.dir, &p.doc, "body");
    list.as_array().unwrap().iter().map(|v| (v["at"].clone(), v["part"].clone())).collect()
}

fn bone(name: &str, extra: Value) -> Value {
    let mut b = json!({ "layer": "body", "name": name });
    b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "bone": b })
}

#[test]
fn bones_are_placed_rehung_and_renamed_on_a_mesh_of_several_parts() {
    let root = tempfile::tempdir().unwrap();
    let mut p = parts(&root);
    let shown = info(&p, "body");
    assert_eq!((shown["editable"].clone(), shown["auto"].clone()), (json!(true), json!(false)));
    assert_eq!(shown["parts"], json!(["legs", "chest"]));
    assert_eq!(
        shown["bones"],
        json!([
            { "name": "hip", "parent": null, "at": [0.5, 0.6] },
            { "name": "neck", "parent": "hip", "at": [0.5, 0.45] }
        ])
    );
    let rest = spots(&p);
    assert!(rest.contains(&(json!([0.4, 0.3]), json!("chest"))), "{rest:?}");
    act(&mut p, bone("tail", json!({ "parent": "hip", "at": [0.8, 0.8] }))).unwrap();
    act(&mut p, bone("neck", json!({ "parent": "" }))).unwrap();
    act(&mut p, bone("hip", json!({ "at": [0.5, 0.7] }))).unwrap();
    let bones = info(&p, "body")["bones"].clone();
    assert_eq!(bones[1], json!({ "name": "neck", "parent": null, "at": [0.5, 0.45] }));
    assert_eq!((bones[0]["at"].clone(), bones[2]["at"].clone()), (json!([0.5, 0.7]), json!([0.8, 0.8])));
    assert_eq!(spots(&p), rest);
    let chest = wprs_editor::rigs::vertices(&p.dir, &p.doc, "body")[4]["weights"].clone();
    assert_eq!(chest, json!({ "neck": 1.0 }));
    let clip = json!([{ "id": "nod", "bones": { "neck": { "rotation": [[0, 0], [1, 20]] } } }]);
    let base = "/scene/layers/0";
    for (key, value) in [
        ("clips", clip),
        ("ik", json!([{ "chain": ["hip", "neck"], "target": "bone:neck", "limits": { "neck": [-10, 10] } }])),
        ("physics", json!([{ "bone": "neck" }])),
        ("hit", json!(["neck"])),
    ] {
        act(&mut p, json!({ "set": { "pointer": format!("{base}/{key}"), "value": value } })).unwrap();
    }
    let hat = json!({ "id": "hat", "type": "solid", "attach": { "layer": "body", "bone": "neck" } });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": hat } })).unwrap();
    act(&mut p, bone("neck", json!({ "to": "spine" }))).unwrap();
    let layer = &p.doc["scene"]["layers"][0];
    assert_eq!(layer["clips"][0]["bones"]["spine"]["rotation"][1], json!([1, 20]));
    assert_eq!(
        (layer["ik"][0]["chain"][1].clone(), layer["ik"][0]["target"].clone()),
        (json!("spine"), json!("bone:spine"))
    );
    assert_eq!(
        (layer["ik"][0]["limits"]["spine"].clone(), layer["physics"][0]["bone"].clone()),
        (json!([-10, 10]), json!("spine"))
    );
    assert_eq!(
        (layer["hit"].clone(), p.doc["scene"]["layers"][1]["attach"]["bone"].clone()),
        (json!(["spine"]), json!("spine"))
    );
    assert!(p.doc["scene"]["layers"][1]["attach"].get("point").is_none());
    let path = p.doc["scene"]["assets"]["body"]["path"].as_str().unwrap().to_owned();
    let rig = wprs_scene::Rigged::decode(&std::fs::read(p.dir.join(path)).unwrap()).unwrap();
    assert_eq!(
        (rig.shapes.clone(), rig.parts.clone(), rig.pieces.len()),
        (vec!["smile".to_owned()], vec!["legs".to_owned(), "chest".to_owned()], 2)
    );
    assert_eq!(rig.morphs[0], [0.0, 5.0, 0.0, 0.0]);
    act(&mut p, bone("spine", json!({ "remove": true }))).unwrap();
    assert_eq!(info(&p, "body")["bones"].as_array().unwrap().len(), 2);
    assert_eq!(spots(&p), rest);
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(info(&p, "body")["bones"][1]["name"], "spine");
    act(&mut p, json!({ "redo": {} })).unwrap();
    for bad in [
        bone("hip", json!({ "parent": "tail" })),
        bone("hip", json!({ "to": "legs" })),
        bone("legs", json!({ "remove": true })),
        bone("tail", json!({ "parent": "nothing" })),
    ] {
        assert!(act(&mut p, bad).is_err());
    }
}
