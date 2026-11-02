use super::motion::rigged;
use super::{act, ask, layer, undone};
use serde_json::{Value, json};

fn near(v: &Value, want: [f64; 3], slack: f64) -> bool {
    (0..3).all(|k| (v[k].as_f64().unwrap() - want[k]).abs() <= slack)
}

fn bone3d(p: &wprs_editor::Project, extra: Value) -> Value {
    let mut q = json!({ "layer": "doll", "bone": "tip", "clip": "bend", "time": 0.5 });
    extra.as_object().unwrap().iter().for_each(|(k, v)| q[k] = v.clone());
    ask(p, json!({ "bone3d": q }))
}

#[test]
fn model_bones_turn_tip_and_slide_in_the_view_and_key_the_clip() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let still = bone3d(&p, json!({}));
    assert!(near(&still["angles"], [0.0; 3], 0.01) && near(&still["shift"], [0.0; 3], 1e-3), "{still}");
    let turned = bone3d(&p, json!({ "turn": 30 }));
    let tipped = bone3d(&p, json!({ "tilt": [20, 0] }));
    let slid = bone3d(&p, json!({ "slide": [40, 0] }));
    let size = |v: &Value| (0..3).map(|k| v[k].as_f64().unwrap().powi(2)).sum::<f64>().sqrt();
    assert!((size(&turned["angles"]) - 30.0).abs() < 0.5, "{turned}");
    assert!((size(&tipped["angles"]) - 20.0).abs() < 0.5, "{tipped}");
    assert!(size(&slid["shift"]) > 0.01 && near(&slid["angles"], [0.0; 3], 0.01), "{slid}");
    let from = bone3d(&p, json!({ "from": { "angles": [0, 0, 10], "shift": [0, 0.5, 0] } }));
    assert!(near(&from["angles"], [0.0, 0.0, 10.0], 0.01) && near(&from["shift"], [0.0, 0.5, 0.0], 1e-3));
    let at = wprs_editor::actions::layer(&p.doc, "doll").unwrap().0;
    let played = json!({ "set": { "pointer": format!("{at}/animations"), "value": [{ "clip": "bend" }] } });
    act(&mut p, played).unwrap();
    let before_dot = dot(&p);
    let before = p.doc.clone();
    let key = json!({ "layer": "doll", "clip": "bend", "time": 0.5, "bone": "root",
        "angles": turned["angles"], "shift": [0, 0, 0] });
    act(&mut p, json!({ "key": key })).unwrap();
    undone(&mut p, &before);
    let kept = bone3d(&p, json!({ "bone": "root", "clip_time": 0.5 }));
    assert_eq!(kept["angles"], turned["angles"], "the key is read back at the playhead");
    assert!(layer(&p, "doll")["clips"][0]["bones"]["root"]["angles"].is_array());
    assert_ne!(dot(&p), before_dot, "the keyed turn of the root moves the tip on the view");
    assert!(bone3d(&p, json!({ "bone": "nobody" })).is_null());
    assert!(bone3d(&p, json!({ "layer": "nothing" })).is_null());
}

fn dot(p: &wprs_editor::Project) -> Value {
    let stage = ask(p, json!({ "stage3d": { "time": 0.5 } }));
    stage["models"][0]["bones"].as_array().unwrap().iter().find(|b| b["name"] == "tip").unwrap()["at"].clone()
}

#[test]
fn swinging_model_bones_show_their_bound_and_drag_it_on_the_view() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    let swing = json!({ "layer": "doll", "bones": ["tip"], "rotation": [-20, 40] });
    act(&mut p, json!({ "physics": swing })).unwrap();
    let stage = ask(&p, json!({ "stage3d": { "time": 0.0 } }));
    let s = &stage["models"][0]["swing"][0];
    assert_eq!((s["bone"].clone(), s["rotation"].clone()), (json!("tip"), json!([-20.0, 40.0])), "{stage}");
    assert_eq!(s["arc"].as_array().unwrap().len(), 13);
    assert_eq!(stage["models"][0]["clips"], json!(["bend"]));
    for (end, want) in [(0, -20.0), (1, 40.0)] {
        let q = json!({ "layer": "doll", "bone": "tip", "point": s["ends"][end] });
        let got = ask(&p, json!({ "bound3d": q }))["degrees"].as_f64().unwrap();
        assert!((got - want).abs() <= 1.0, "end {end}: {got}");
    }
    let off = json!({ "layer": "doll", "bone": "nobody", "point": [0, 0] });
    assert!(ask(&p, json!({ "bound3d": off })).is_null());
}

#[test]
fn camera_path_keys_and_their_aims_follow_the_cursor_at_their_own_depth() {
    let root = tempfile::tempdir().unwrap();
    let mut p = super::project(&root);
    super::edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    super::edit(&mut p, json!({ "view": { "orbit": [0, 0] } })).unwrap();
    super::edit(&mut p, json!({ "path": { "add": { "id": "tour" } } })).unwrap();
    let near = json!({ "path": "tour", "time": 0, "eye": [0, 0.5, 3], "center": [0, 0, 0] });
    let far = json!({ "path": "tour", "time": 2, "eye": [0, 0.5, -10], "center": [0, 0, -2] });
    super::edit(&mut p, json!({ "path": { "key": near } })).unwrap();
    super::edit(&mut p, json!({ "path": { "key": far } })).unwrap();
    let handles = ask(&p, json!({ "stage3d": { "time": 0 } }))["handles"].clone();
    let keys = ask(&p, json!({ "paths3d": { "time": 0 } }))["paths"][0]["keys"].clone();
    let f = |v: &Value| v.as_f64().unwrap();
    let (a, b) = (f(&keys[0]["reach_eye"]), f(&keys[1]["reach_eye"]));
    assert!((b / a - 7.5).abs() < 0.2, "{a} {b}");
    for (k, field, shown) in [(0, "eye", "at"), (1, "eye", "at"), (1, "center", "aim")] {
        let key = &keys[k];
        let reach = f(&key[format!("reach_{field}")]);
        let moved: Vec<f64> =
            (0..3).map(|i| f(&key[field][i]) + f(&handles["right"][i]) * 30.0 * reach).collect();
        let mut args = json!({ "path": "tour", "time": key["t"] });
        args[field] = json!(moved);
        super::edit(&mut p, json!({ "path": { "key": args } })).unwrap();
        let after = ask(&p, json!({ "paths3d": { "time": 0 } }))["paths"][0]["keys"][k][shown].clone();
        let dx = f(&after[0]) - f(&key[shown][0]);
        assert!((dx - 30.0).abs() < 1.5, "key {k} {field} moved {dx} px for 30");
    }
}

#[test]
fn a_keyed_model_position_is_keyed_again_and_a_bound_one_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let mut p = super::project(&root);
    super::edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let at = wprs_editor::actions::layer(&p.doc, "doll").unwrap().0;
    let keys = json!({ "keys": [[0, [0, 0, 0]], [2, [2, 0, 0]]] });
    act(&mut p, json!({ "set": { "pointer": format!("{at}/position"), "value": keys } })).unwrap();
    let shown = ask(&p, json!({ "stage3d": { "time": 1.0 } }))["models"][0].clone();
    assert_eq!(shown["held"]["position"], "keyed");
    assert!(near(&shown["now"]["position"], [1.0, 0.0, 0.0], 1e-3), "{shown}");
    assert!(near(&shown["now"]["scale"], [1.0; 3], 1e-3) && near(&shown["now"]["rotation"], [0.0; 3], 1e-2));
    let set = json!({ "model": { "set": { "layer": "doll", "position": [5, 0, 0] } } });
    assert!(super::edit(&mut p, set.clone()).is_err(), "a plain set never drops the keys");
    let before = p.doc.clone();
    act(
        &mut p,
        json!({ "keyframe": { "pointer": format!("{at}/position"), "time": 1, "value": [1, 3, 0] } }),
    )
    .unwrap();
    undone(&mut p, &before);
    assert_eq!(layer(&p, "doll")["position"]["keys"].as_array().unwrap().len(), 3);
    let bound = json!({ "prop": "spot", "map": { "a": [0, 0, 0] } });
    act(&mut p, json!({ "set": { "pointer": format!("{at}/position"), "value": bound } })).unwrap();
    assert_eq!(ask(&p, json!({ "stage3d": { "time": 1.0 } }))["models"][0]["held"]["position"], "bound");
    assert!(super::edit(&mut p, set).is_err());
    let turned = json!({ "model": { "set": { "layer": "doll", "rotation": [0, 30, 0] } } });
    super::edit(&mut p, turned).unwrap();
    let now = &ask(&p, json!({ "stage3d": { "time": 0.0 } }))["models"][0]["now"];
    assert!(near(&now["rotation"], [0.0, 30.0, 0.0], 0.01), "{now}");
}
