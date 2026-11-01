use super::kit::{act, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn bone(name: &str, extra: Value) -> Value {
    let mut b = json!({ "layer": "net", "name": name });
    b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "bone": b })
}

fn set(pointer: &str, value: Value) -> Value {
    json!({ "set": { "pointer": format!("/scene/layers/0/{pointer}"), "value": value } })
}

fn broken(p: &Project) -> Vec<String> {
    let all = wprs_editor::package::problems(&p.dir, &p.doc);
    all.into_iter().filter(|q| !q.warning && q.pointer.contains("/layers/")).map(|q| q.message).collect()
}

fn layer(p: &Project) -> &Value {
    &p.doc["scene"]["layers"][0]
}

fn dressed(root: &tempfile::TempDir) -> Project {
    let mut p = rigged(root);
    act(&mut p, bone("hand", json!({ "parent": "arm", "at": [0.8, 0.5] }))).unwrap();
    act(&mut p, bone("tail", json!({ "parent": "root", "at": [0.2, 0.8] }))).unwrap();
    let chain = json!({ "layer": "net", "root": "root", "tip": "hand", "limits": { "hand": [-30, 30] } });
    act(&mut p, json!({ "ik": chain })).unwrap();
    act(&mut p, json!({ "physics": { "layer": "net", "bones": ["hand"] } })).unwrap();
    for id in ["swing", "wave"] {
        act(&mut p, json!({ "clip": { "layer": "net", "id": id, "add": true } })).unwrap();
        let k = json!({ "layer": "net", "clip": id, "time": 0.5, "bone": "hand", "rotation": 30 });
        act(&mut p, json!({ "key": k })).unwrap();
    }
    let seeded = json!({ "layer": "net", "clip": "wave", "time": 0, "bone": "root", "remove": true });
    act(&mut p, json!({ "key": seeded })).unwrap();
    act(&mut p, set("animations", json!([{ "clip": "swing", "bones": ["hand"] }, { "clip": "swing" }])))
        .unwrap();
    act(&mut p, set("rules", json!([{ "bone": "tail", "parent": "hand", "weight": 1 }]))).unwrap();
    act(&mut p, set("hit", json!(["hand", "arm"]))).unwrap();
    let hat = json!({ "id": "hat", "type": "solid", "color": "#ffffff", "attach": { "layer": "net", "bone": "hand" } });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": hat } })).unwrap();
    assert_eq!(broken(&p), Vec::<String>::new());
    p
}

#[test]
fn removing_a_bone_drops_what_names_it_in_the_same_step() {
    let root = tempfile::tempdir().unwrap();
    let mut p = dressed(&root);
    let before = p.doc.clone();
    act(&mut p, bone("hand", json!({ "remove": true }))).unwrap();
    assert_eq!(broken(&p), Vec::<String>::new());
    let l = layer(&p);
    assert_eq!(l["ik"], json!([{ "chain": ["root", "arm"], "target": [88.0, 20.0], "limits": {} }]));
    assert!(l.get("physics").is_none() && l.get("rules").is_none());
    assert_eq!((l["hit"].clone(), l["animations"].clone()), (json!(["arm"]), json!([{ "clip": "swing" }])));
    assert!(l["clips"][0]["bones"].get("hand").is_none());
    assert_eq!(l["clips"][1]["bones"], json!({ "arm": { "rotation": [[0.0, 0]] } }));
    assert!(p.doc["scene"]["layers"][1].get("attach").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc, before);
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert!(layer(&p).get("physics").is_none());
}

#[test]
fn rehanging_a_bone_drops_the_chains_and_rules_it_breaks() {
    let root = tempfile::tempdir().unwrap();
    let mut p = dressed(&root);
    act(&mut p, bone("arm", json!({ "parent": "tail" }))).unwrap();
    assert_eq!(broken(&p), Vec::<String>::new());
    assert!(layer(&p).get("ik").is_none() && layer(&p).get("rules").is_none());
    assert_eq!(layer(&p)["physics"], json!([{ "bone": "hand", "mode": "spring" }]));
    act(&mut p, json!({ "undo": {} })).unwrap();
    act(&mut p, bone("arm", json!({ "parent": "" }))).unwrap();
    assert_eq!(broken(&p), Vec::<String>::new());
    assert!(layer(&p).get("ik").is_none());
    assert_eq!(layer(&p)["rules"], json!([{ "bone": "tail", "parent": "hand", "weight": 1 }]));
    act(&mut p, json!({ "undo": {} })).unwrap();
    act(&mut p, bone("tail", json!({ "parent": "hand" }))).unwrap();
    assert_eq!(layer(&p)["ik"][0]["chain"], json!(["root", "arm", "hand"]));
    assert_eq!(broken(&p), Vec::<String>::new());
}

#[test]
fn removing_a_bone_or_point_leaves_chains_without_limits_as_they_were() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, bone("tail", json!({ "parent": "root", "at": [0.2, 0.8] }))).unwrap();
    act(&mut p, json!({ "ik": { "layer": "net", "root": "root", "tip": "arm" } })).unwrap();
    let chain = layer(&p)["ik"].clone();
    assert!(chain[0].get("limits").is_none());
    let pin = json!({ "point": { "layer": "net", "name": "pin", "at": [0.5, 0.3] } });
    act(&mut p, pin).unwrap();
    act(&mut p, json!({ "point": { "layer": "net", "name": "pin", "remove": true } })).unwrap();
    assert_eq!((broken(&p), layer(&p)["ik"].clone()), (Vec::<String>::new(), chain.clone()));
    act(&mut p, bone("tail", json!({ "remove": true }))).unwrap();
    assert_eq!((broken(&p), layer(&p)["ik"].clone()), (Vec::<String>::new(), chain));
}

#[test]
fn renaming_and_rehanging_a_bone_at_once_drops_the_chains_it_breaks() {
    let root = tempfile::tempdir().unwrap();
    let mut p = dressed(&root);
    act(&mut p, bone("arm", json!({ "to": "limb", "parent": "tail" }))).unwrap();
    assert_eq!(broken(&p), Vec::<String>::new());
    assert!(layer(&p).get("ik").is_none() && layer(&p).get("rules").is_none());
    assert_eq!(layer(&p)["hit"], json!(["hand", "limb"]));
    act(&mut p, json!({ "undo": {} })).unwrap();
    act(&mut p, bone("arm", json!({ "to": "limb", "at": [0.6, 0.5] }))).unwrap();
    assert_eq!(layer(&p)["ik"][0]["chain"], json!(["root", "limb", "hand"]));
    assert_eq!(broken(&p), Vec::<String>::new());
}
