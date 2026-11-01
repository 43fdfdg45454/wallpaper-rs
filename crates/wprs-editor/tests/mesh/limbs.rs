use super::kit::{act, project, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;

fn ik(extra: Value) -> Value {
    let mut k = json!({ "layer": "net" });
    k.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "ik": k })
}

fn chain(p: &Project) -> Value {
    p.doc["scene"]["layers"][0]["ik"][0].clone()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

#[test]
fn chains_are_added_aimed_limited_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let hand = json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.9, 0.5] } });
    act(&mut p, hand).unwrap();
    act(&mut p, ik(json!({ "root": "root", "tip": "hand", "limits": { "arm": [-45, 45] } }))).unwrap();
    assert_eq!(
        chain(&p),
        json!({ "chain": ["root", "arm", "hand"], "target": [104.0, 20.0], "limits": { "arm": [-45.0, 45.0] } })
    );
    let shown = query(&p, "net", json!({ "handles": true }))["ik"][0].clone();
    assert_eq!(shown["at"], json!([[0.1, 0.5], [0.5, 0.5], [0.9, 0.5]]));
    assert_eq!((shown["target"].clone(), shown["follows"].clone()), (json!([1.3, 0.5]), json!("point")));
    let arc = &shown["limits"][0];
    assert_eq!((arc["bone"].clone(), arc["at"].clone()), (json!("arm"), json!([0.5, 0.5])));
    assert_eq!((num(&arc["rest"]), num(&arc["min"]), num(&arc["max"])), (0.0, -45.0, 45.0));
    let aim = json!({ "index": 0, "target": "bone:arm", "bend": "counterclockwise", "weight": 0.5, "unlimit": ["arm"] });
    act(&mut p, ik(aim)).unwrap();
    let shown = query(&p, "net", json!({ "handles": true }))["ik"][0].clone();
    assert_eq!((shown["target"].clone(), shown["follows"].clone()), (json!([0.5, 0.5]), json!("bone")));
    assert_eq!(
        chain(&p),
        json!({ "chain": ["root", "arm", "hand"], "target": "bone:arm", "bend": "counterclockwise", "weight": 0.5 })
    );
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(chain(&p)["target"], json!([104.0, 20.0]));
    act(&mut p, json!({ "redo": {} })).unwrap();
    act(&mut p, ik(json!({ "index": 0, "root": "arm", "tip": "hand", "target": [10, 5] }))).unwrap();
    assert_eq!(
        (chain(&p)["chain"].clone(), chain(&p)["target"].clone()),
        (json!(["arm", "hand"]), json!([10, 5]))
    );
    for bad in [
        json!({ "root": "root", "tip": "root" }),
        json!({ "root": "hand", "tip": "root" }),
        json!({ "root": "root", "tip": "nothing" }),
        json!({ "root": "root" }),
        json!({ "root": "root", "tip": "arm", "limits": { "hand": [0, 10] } }),
        json!({ "root": "root", "tip": "arm", "limits": { "arm": [50, -50] } }),
        json!({ "root": "root", "tip": "arm", "target": "nowhere" }),
        json!({ "root": "root", "tip": "arm", "target": "layer:net" }),
        json!({ "index": 4, "weight": 0.5 }),
        json!({ "index": 0, "weight": 2 }),
        json!({ "remove": true }),
    ] {
        assert!(act(&mut p, ik(bad.clone())).is_err(), "{bad}");
    }
    act(&mut p, ik(json!({ "index": 0, "remove": true }))).unwrap();
    assert!(p.doc["scene"]["layers"][0].get("ik").is_none());
}

#[test]
fn the_wind_is_set_by_angle_strength_and_gusts() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let wind = |p: &Project| p.doc["scene"]["canvas"]["wind"].clone();
    act(&mut p, json!({ "wind": { "angle": 90, "strength": 40, "gusts": 0.5 } })).unwrap();
    assert_eq!((wind(&p), p.doc["scene"]["canvas"]["gusts"].clone()), (json!([0.0, 40.0]), json!(0.5)));
    let arrow = wprs_editor::rigs::wind_arrow(&p.doc);
    assert_eq!((num(&arrow["angle"]), num(&arrow["strength"]), num(&arrow["gusts"])), (90.0, 40.0, 0.5));
    act(&mut p, json!({ "wind": { "strength": 20 } })).unwrap();
    assert_eq!(wind(&p), json!([0.0, 20.0]));
    act(&mut p, json!({ "wind": { "angle": 180 } })).unwrap();
    assert_eq!(wind(&p), json!([-20.0, 0.0]));
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_eq!(wind(&p), json!([0.0, 20.0]));
    act(&mut p, json!({ "redo": {} })).unwrap();
    for bad in [json!({ "strength": -1 }), json!({ "gusts": 2 }), json!({})] {
        assert!(act(&mut p, json!({ "wind": bad })).is_err());
    }
    act(&mut p, json!({ "wind": { "off": true } })).unwrap();
    let canvas = &p.doc["scene"]["canvas"];
    assert!(canvas.get("wind").is_none() && canvas.get("gusts").is_none());
    assert_eq!(wprs_editor::rigs::wind_arrow(&p.doc)["fixed"], json!(true));
}
