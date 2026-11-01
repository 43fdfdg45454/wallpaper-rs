use super::kit::{act, info, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;
use wprs_editor::actions::{Action, AttachEdit, PointEdit};

fn point(name: &str, extra: Value) -> Value {
    let mut b = json!({ "layer": "net", "name": name });
    b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "point": b })
}

fn attach(extra: Value) -> Value {
    let mut a = json!({ "layer": "hat" });
    a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "attach": a })
}

fn points(p: &Project) -> Vec<Value> {
    let all = info(p, "net")["points"].as_array().unwrap().clone();
    all.into_iter().filter(|q| q["part"] == false).collect()
}

fn hatted(root: &tempfile::TempDir) -> Project {
    let mut p = rigged(root);
    let hat = json!({ "id": "hat", "type": "solid", "color": "#ffffff" });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": hat } })).unwrap();
    p
}

#[test]
fn points_are_placed_moved_rehung_renamed_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = hatted(&root);
    act(&mut p, point("badge", json!({ "at": [0.6, 0.25] }))).unwrap();
    assert_eq!(points(&p), [json!({ "name": "badge", "at": [0.6, 0.25], "part": false, "parent": "arm" })]);
    act(&mut p, point("badge", json!({ "at": [0.2, 0.75], "parent": "root" }))).unwrap();
    assert_eq!(points(&p), [json!({ "name": "badge", "at": [0.2, 0.75], "part": false, "parent": "root" })]);
    act(&mut p, attach(json!({ "to": "net", "point": "badge" }))).unwrap();
    let aim = json!({ "layer": "net", "root": "root", "tip": "arm", "target": "bone:badge" });
    act(&mut p, json!({ "ik": aim })).unwrap();
    act(&mut p, point("badge", json!({ "to": "pin" }))).unwrap();
    assert_eq!(p.doc["scene"]["layers"][1]["attach"]["point"], json!("pin"));
    assert_eq!(p.doc["scene"]["layers"][0]["ik"][0]["target"], json!("bone:pin"));
    for bad in [
        point("pin", json!({ "to": "arm" })),
        point("pin", json!({ "to": "a/b" })),
        point("pin", json!({ "parent": "nothing" })),
        point("ghost", json!({})),
        point("ghost", json!({ "remove": true })),
        point("arm", json!({ "remove": true })),
        point("root", json!({ "at": [0.5, 0.5] })),
    ] {
        assert!(act(&mut p, bad.clone()).is_err(), "{bad}");
    }
    let at = Some([f32::NAN, 0.5]);
    let odd =
        PointEdit { layer: "net".into(), name: "pin".into(), at, parent: None, to: None, remove: false };
    assert!(p.act(&Action::Point(odd)).is_err());
    act(&mut p, point("pin", json!({ "remove": true }))).unwrap();
    assert!(points(&p).is_empty());
    assert!(p.doc["scene"]["layers"][1].get("attach").is_none());
    assert!(p.doc["scene"]["layers"][0].get("ik").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(points(&p).len(), 1);
    assert_eq!(p.doc["scene"]["layers"][1]["attach"]["point"], json!("pin"));
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert!(points(&p).is_empty());
}

#[test]
fn a_dropped_layer_moves_its_own_point_and_takes_it_away_when_it_leaves() {
    let root = tempfile::tempdir().unwrap();
    let mut p = hatted(&root);
    act(&mut p, attach(json!({ "to": "net", "point": [0.6, 0.25] }))).unwrap();
    act(&mut p, attach(json!({ "point": [0.3, 0.75] }))).unwrap();
    assert_eq!(
        points(&p),
        [json!({ "name": "hat_point", "at": [0.3, 0.75], "part": false, "parent": "root" })]
    );
    act(&mut p, attach(json!({ "bone": "arm" }))).unwrap();
    assert!(points(&p).is_empty());
    act(&mut p, attach(json!({ "point": [0.6, 0.25] }))).unwrap();
    act(&mut p, attach(json!({ "detach": true }))).unwrap();
    assert!(points(&p).is_empty() && p.doc["scene"]["layers"][1].get("attach").is_none());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(points(&p).len(), 1);
}

#[test]
fn a_nudge_moves_an_attached_layer_along_its_bone() {
    let root = tempfile::tempdir().unwrap();
    let mut p = hatted(&root);
    act(&mut p, attach(json!({ "to": "net", "bone": "arm", "offset": [5, 0] }))).unwrap();
    act(&mut p, attach(json!({ "nudge": [3, -4] }))).unwrap();
    assert_eq!(p.doc["scene"]["layers"][1]["attach"]["offset"], json!([8.0, -4.0]));
    let mut far: AttachEdit = serde_json::from_value(json!({ "layer": "hat", "nudge": [0, 0] })).unwrap();
    far.nudge = Some([f32::INFINITY, 0.0]);
    assert!(p.act(&Action::Attach(far)).is_err());
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(p.doc["scene"]["layers"][1]["attach"]["offset"], json!([5.0, 0.0]));
}

#[test]
fn a_point_on_the_tip_bone_moves_neither_chain_ends_nor_swinging_tips() {
    let root = tempfile::tempdir().unwrap();
    let mut p = hatted(&root);
    act(&mut p, json!({ "ik": { "layer": "net", "root": "root", "tip": "arm" } })).unwrap();
    act(&mut p, json!({ "physics": { "layer": "net", "bones": ["arm"] } })).unwrap();
    let before = query(&p, "net", json!({ "handles": true }));
    act(&mut p, attach(json!({ "to": "net", "point": [0.9, 0.15] }))).unwrap();
    assert_eq!(points(&p)[0]["parent"], json!("arm"));
    let after = query(&p, "net", json!({ "handles": true }));
    assert_eq!(
        (after["ik"][0]["end"].clone(), after["bodies"][0]["tip"].clone()),
        (json!([0.9, 0.5]), json!([0.9, 0.5]))
    );
    assert_eq!(before, after);
}
