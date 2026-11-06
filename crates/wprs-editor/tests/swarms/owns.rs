use super::*;

fn layered(p: &mut Project) -> String {
    edit(p, "point", json!({ "point": 0, "at": [100, 50] })).unwrap();
    edit(p, "point", json!({ "point": 1, "at": [200, 80] })).unwrap();
    let id = handles(p)["layer"].as_str().unwrap().to_owned();
    let keys = json!({ "keys": [[0, [100, 50]], [4, [900, 50]]] });
    act(p, json!({ "set": { "pointer": "/scene/particles/flakes/points/0/at", "value": keys } })).unwrap();
    id
}

fn own(p: &mut Project, id: &str, points: Value) -> anyhow::Result<()> {
    act(p, json!({ "layer": { "instance": { "layer": id, "values": { "points": points } } } }))
}

fn instance(p: &Project, id: &str) -> Value {
    let (path, _) = wprs_editor::actions::layer(&p.doc, id).unwrap();
    p.doc.pointer(&format!("{path}/instance")).cloned().unwrap_or(Value::Null)
}

#[test]
fn moving_one_point_of_a_layer_leaves_the_others_to_the_system() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let id = layered(&mut p);
    own(&mut p, &id, json!({ "1": [500, 300] })).unwrap();
    assert_eq!(instance(&p, &id)["points"], json!([null, [500, 300]]));
    let h = wprs_editor::swarms::handles(&p.dir, &p.doc, "flakes", Some(&id), 2.0);
    assert_eq!(h["points"][0]["local"], json!([500.0, 50.0]), "p0 still follows its keys");
    assert!(h["points"][0].get("own").is_none(), "{}", h["points"][0]);
    assert_eq!((&h["points"][1]["local"], &h["points"][1]["own"]), (&json!([500.0, 300.0]), &json!(true)));
    own(&mut p, &id, json!({ "0": { "keys": [[0, [1, 2]], [1, [3, 4]]] } })).unwrap();
    own(&mut p, &id, json!({ "1": [600, 300] })).unwrap();
    assert_eq!(instance(&p, &id)["points"][0]["keys"], json!([[0, [1, 2]], [1, [3, 4]]]), "own keys stay");
    own(&mut p, &id, json!({ "0": null, "1": null })).unwrap();
    assert_eq!(instance(&p, &id), Value::Null, "no point left");
}

#[test]
fn a_point_of_a_layer_takes_a_known_index_and_a_pair() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let id = layered(&mut p);
    let far = own(&mut p, &id, json!({ "8": [1, 2] })).unwrap_err().to_string();
    assert!(far.contains("a control point goes from 0 to 7, not 8"), "{far}");
    assert!(own(&mut p, &id, json!({ "x": [1, 2] })).is_err());
    assert!(own(&mut p, &id, json!({ "1": [1, 2, 3] })).is_err());
    assert_eq!(instance(&p, &id), Value::Null);
}
