use super::kit::{act, info, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn physics(extra: Value) -> Value {
    let mut b = json!({ "layer": "net" });
    b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "physics": b })
}

fn bodies(p: &Project) -> Value {
    p.doc["scene"]["layers"][0]["physics"].clone()
}

fn names(p: &Project) -> Vec<(Value, Value)> {
    let bones = info(p, "net")["bones"].as_array().unwrap().clone();
    bones.iter().map(|b| (b["name"].clone(), b["parent"].clone())).collect()
}

#[test]
fn bones_swing_with_their_settings_and_a_rope_is_drawn() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let tuned = json!({ "bones": ["arm"], "stiffness": 0.2, "gravity": 3, "rotation": [-30, 30] });
    act(&mut p, physics(tuned)).unwrap();
    act(&mut p, physics(json!({ "bones": ["arm"], "mode": "rigid", "damping": 0.9 }))).unwrap();
    assert_eq!(
        bodies(&p),
        json!([{ "bone": "arm", "mode": "rigid", "stiffness": 0.2, "damping": 0.9, "gravity": 3.0, "rotation": [-30.0, 30.0] }])
    );
    let rope = json!({ "bones": ["tail"], "rope": [[0.5, 0.6], [0.6, 0.8], [0.7, 0.9]], "parent": "arm", "wind": 0.5 });
    act(&mut p, physics(rope)).unwrap();
    assert_eq!(bodies(&p)[1], json!({ "bone": "tail_1", "mode": "rope", "wind": 0.5 }));
    assert_eq!(
        names(&p)[2..],
        [
            (json!("tail_1"), json!("arm")),
            (json!("tail_2"), json!("tail_1")),
            (json!("tail_3"), json!("tail_2"))
        ]
    );
    let shown = query(&p, "net", json!({ "handles": true }))["bodies"].clone();
    assert_eq!((shown[0]["at"].clone(), shown[0]["gravity"].as_f64()), (json!([0.5, 0.5]), Some(3.0)));
    assert_eq!((shown[1]["at"].clone(), shown[1]["tip"].clone()), (json!([0.5, 0.6]), json!([0.6, 0.8])));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!((names(&p).len(), bodies(&p).as_array().unwrap().len()), (2, 1));
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(names(&p).len(), 5);
    for bad in [
        json!({ "bones": ["nothing"] }),
        json!({ "bones": [] }),
        json!({ "bones": ["arm"], "stiffness": 2 }),
        json!({ "bones": ["arm"], "gravity": 11 }),
        json!({ "bones": ["arm"], "rotation": [10, -10] }),
        json!({ "bones": ["root"], "remove": true }),
        json!({ "bones": ["rope"], "rope": [[0.5, 0.5]] }),
        json!({ "bones": ["a", "b"], "rope": [[0.5, 0.5], [0.6, 0.6]] }),
        json!({ "bones": ["tail"], "rope": [[0.5, 0.5], [0.6, 0.6]] }),
        json!({ "bones": ["vine"], "rope": [[0.5, 0.5], [0.6, 0.6]], "parent": "nothing" }),
    ] {
        assert!(act(&mut p, physics(bad.clone())).is_err(), "{bad}");
    }
    act(&mut p, physics(json!({ "bones": ["arm", "tail_1"], "remove": true }))).unwrap();
    assert!(p.doc["scene"]["layers"][0].get("physics").is_none());
}
