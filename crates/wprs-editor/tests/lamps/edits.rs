use super::{act, layer, light, project, undone};
use serde_json::json;

#[test]
fn each_light_type_starts_with_its_own_defaults() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    for kind in ["point", "spot", "tube", "directional"] {
        let before = p.doc.clone();
        light(&mut p, json!({ "add": { "light": kind } })).unwrap();
        undone(&mut p, &before);
        let l = layer(&p, kind);
        assert_eq!((l["type"].as_str(), l["light"].as_str()), (Some("light"), Some(kind)));
        assert_eq!(l["transform"]["position"], json!([960.0, 540.0]));
    }
    assert_eq!(layer(&p, "spot")["cone"], json!([25, 45]));
    assert_eq!(layer(&p, "tube")["length"], json!(400));
    assert_eq!(layer(&p, "directional")["transform"]["tilt"], json!([50, 0]));
    act(&mut p, json!({ "add_layer": { "kind": "light", "source": "spot", "id": "menu" } })).unwrap();
    assert_eq!(layer(&p, "menu")["light"], "spot");
    assert!(act(&mut p, json!({ "add_layer": { "kind": "light", "source": "area" } })).is_err());
}

#[test]
fn setting_a_light_checks_its_values_and_fields() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    light(&mut p, json!({ "add": { "light": "spot", "id": "s" } })).unwrap();
    let before = p.doc.clone();
    let set = json!({ "layer": "s", "intensity": 3, "radius": 600, "z": 40, "cone": [10, 30], "color": "#ff8800", "tilt": 20 });
    light(&mut p, json!({ "set": set })).unwrap();
    undone(&mut p, &before);
    let s = layer(&p, "s");
    assert_eq!(
        (s["intensity"].as_f64(), s["cone"].clone(), s["color"].clone()),
        (Some(3.0), json!([10.0, 30.0]), json!("#ff8800"))
    );
    assert_eq!(s["transform"]["tilt"], json!([20.0, 0]));
    for bad in [
        json!({ "layer": "s", "intensity": 101 }),
        json!({ "layer": "s", "radius": 0 }),
        json!({ "layer": "s", "cone": [40, 20] }),
        json!({ "layer": "s", "length": 10 }),
        json!({ "layer": "s", "color": "red-ish" }),
        json!({ "layer": "s", "tilt": 95 }),
        json!({ "layer": "logo", "intensity": 1 }),
        json!({ "layer": "nothing", "intensity": 1 }),
    ] {
        assert!(light(&mut p, json!({ "set": bad.clone() })).is_err(), "{bad}");
    }
    light(&mut p, json!({ "set": { "layer": "s", "light": "tube" } })).unwrap();
    let t = layer(&p, "s");
    assert!(t.get("cone").is_none() && t["light"] == "tube");
}

#[test]
fn positions_and_aims_are_given_on_the_canvas() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let group = json!({ "id": "g", "type": "group", "transform": { "position": [100, 50], "rotation": 90 }, "layers": [] });
    act(&mut p, json!({ "add": { "pointer": "/scene/layers/-", "value": group } })).unwrap();
    light(&mut p, json!({ "add": { "light": "spot", "id": "s", "into": "g", "at": [0, 0] } })).unwrap();
    light(&mut p, json!({ "set": { "layer": "s", "at": [100, 150] } })).unwrap();
    let at = layer(&p, "s")["transform"]["position"].clone();
    assert_eq!(at, json!([100.0, 0.0]));
    light(&mut p, json!({ "set": { "layer": "s", "aim": [100, 250] } })).unwrap();
    let turn = layer(&p, "s")["transform"]["rotation"].as_f64().unwrap();
    assert!((turn + 90.0).abs() < 0.01, "{turn}");
}

#[test]
fn removing_solo_and_mute_tidy_the_preview_lists() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    light(&mut p, json!({ "add": { "id": "a" } })).unwrap();
    light(&mut p, json!({ "add": { "id": "b" } })).unwrap();
    let before = p.doc.clone();
    light(&mut p, json!({ "mute": { "layer": "a", "muted": true } })).unwrap();
    undone(&mut p, &before);
    light(&mut p, json!({ "solo": { "layer": "b" } })).unwrap();
    let mut scene = p.doc["scene"].clone();
    wprs_editor::rigs::preview(&mut scene, &json!({}));
    let shown = |id: &str| {
        scene["layers"].as_array().unwrap().iter().find(|l| l["id"] == id).unwrap()["visible"] != false
    };
    assert!(!shown("a") && shown("b") && shown("logo"));
    light(&mut p, json!({ "remove": { "layer": "b" } })).unwrap();
    assert!(p.doc.pointer("/scene/ext/editor/lights/solo").is_none());
    light(&mut p, json!({ "remove": { "layer": "a" } })).unwrap();
    assert_eq!(p.doc.pointer("/scene/ext/editor/lights/muted"), Some(&json!([])));
    assert!(light(&mut p, json!({ "remove": { "layer": "logo" } })).is_err());
    assert!(light(&mut p, json!({ "mute": { "layer": "logo", "muted": true } })).is_err());
}
