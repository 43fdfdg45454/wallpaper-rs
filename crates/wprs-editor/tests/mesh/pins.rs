use super::kit::{act, info, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn attach(extra: Value) -> Value {
    let mut a = json!({ "layer": "hat" });
    a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "attach": a })
}

fn pin(p: &Project) -> Value {
    p.doc["scene"]["layers"][1]["attach"].clone()
}

#[test]
fn layers_follow_bones_and_points_placed_on_the_mesh() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(
        &mut p,
        json!({ "add": { "pointer": "/scene/layers/-", "value": { "id": "hat", "type": "solid" } } }),
    )
    .unwrap();
    act(&mut p, attach(json!({ "to": "net", "bone": "arm", "offset": [5, 0], "inherit": ["position"] })))
        .unwrap();
    assert_eq!(
        pin(&p),
        json!({ "layer": "net", "bone": "arm", "offset": [5.0, 0.0], "inherit": ["position"] })
    );
    act(&mut p, attach(json!({ "point": [0.3, 0.5] }))).unwrap();
    assert_eq!(
        pin(&p),
        json!({ "layer": "net", "point": "hat_point", "offset": [5.0, 0.0], "inherit": ["position"] })
    );
    let shown = info(&p, "net");
    assert_eq!(shown["bones"].as_array().unwrap().len(), 2);
    let points = shown["points"].as_array().unwrap();
    assert!(points.contains(&json!({ "name": "hat_point", "at": [0.3, 0.5], "part": false })), "{points:?}");
    let path = p.doc["scene"]["assets"]["net"]["path"].as_str().unwrap().to_owned();
    let rig = wprs_scene::Rigged::decode(&std::fs::read(p.dir.join(path)).unwrap()).unwrap();
    let node = rig.joint("hat_point").unwrap();
    assert!(rig.joints[node].parent.is_some() && rig.palette.iter().all(|(j, _)| *j as usize != node));
    act(&mut p, attach(json!({ "point": "hat_point", "inherit": ["position", "rotation", "scale"] })))
        .unwrap();
    assert_eq!(pin(&p), json!({ "layer": "net", "point": "hat_point", "offset": [5.0, 0.0] }));
    act(&mut p, json!({ "undo": {} })).unwrap();
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(pin(&p)["bone"], json!("arm"));
    assert_eq!(
        info(&p, "net")["points"].as_array().unwrap().iter().filter(|q| q["name"] == "hat_point").count(),
        0
    );
    act(&mut p, json!({ "redo": {} })).unwrap();
    for bad in [
        json!({ "to": "hat", "bone": "arm" }),
        json!({ "layer": "net", "to": "net", "bone": "arm" }),
        json!({ "bone": "nothing" }),
        json!({ "bone": "arm", "point": "hat_point" }),
        json!({ "point": "nothing" }),
    ] {
        assert!(act(&mut p, attach(bad.clone())).is_err(), "{bad}");
    }
    act(&mut p, attach(json!({ "detach": true }))).unwrap();
    assert!(pin(&p).is_null());
    assert!(act(&mut p, attach(json!({ "detach": true }))).is_err());
    assert!(act(&mut p, attach(json!({ "to": "net" }))).is_err());
}
