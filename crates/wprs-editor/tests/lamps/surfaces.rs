use super::{act, layer, project, undone};
use serde_json::json;

#[test]
fn the_lighting_block_takes_pictures_and_numbers_in_range() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    act(&mut p, json!({ "lighting": { "layer": "logo", "normal": "pic", "roughness": 0.2, "emissive": 2, "strength": 0.5 } }))
        .unwrap();
    undone(&mut p, &before);
    let l = &layer(&p, "logo")["lighting"];
    assert_eq!(
        (l["normal"].as_str(), l["roughness"].as_f64(), l["emissive"].as_f64()),
        (Some("pic_linear"), Some(0.2), Some(2.0))
    );
    assert_eq!(p.doc["scene"]["assets"]["pic_linear"], json!({ "path": "pic.png", "linear": true }));
    assert!(
        p.doc["scene"]["assets"]["pic"].get("linear").is_none(),
        "the picture of the layer keeps its colors"
    );
    act(&mut p, json!({ "lighting": { "layer": "logo", "normal": "", "env": "pic" } })).unwrap();
    let l = &layer(&p, "logo")["lighting"];
    assert!(l.get("normal").is_none() && l["env"] == "pic" && l["strength"] == 0.5);
    for bad in [
        json!({ "layer": "logo", "roughness": 2 }),
        json!({ "layer": "logo", "emissive": 17 }),
        json!({ "layer": "logo", "normal": "song" }),
        json!({ "layer": "logo", "env": "missing" }),
    ] {
        assert!(act(&mut p, json!({ "lighting": bad.clone() })).is_err(), "{bad}");
    }
    act(&mut p, json!({ "light": { "add": { "id": "l" } } })).unwrap();
    assert!(act(&mut p, json!({ "lighting": { "layer": "l", "roughness": 0.5 } })).is_err());
    act(&mut p, json!({ "lighting": { "layer": "logo", "off": true } })).unwrap();
    assert!(layer(&p, "logo").get("lighting").is_none());
}

#[test]
fn ambient_and_sky_are_canvas_colors() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    act(&mut p, json!({ "ambient": { "ambient": "#202020", "sky": "#8090ff" } })).unwrap();
    undone(&mut p, &before);
    assert_eq!(p.doc["scene"]["canvas"]["sky"], "#8090ff");
    let shown = wprs_editor::lamps::hemisphere(&p.doc);
    assert!(shown["sphere"].as_str().unwrap().starts_with("data:image/png;base64,"));
    assert!(shown["sky"][2].as_f64().unwrap() > 0.99);
    assert!(act(&mut p, json!({ "ambient": { "ambient": "dark" } })).is_err());
    act(&mut p, json!({ "ambient": { "clear": true } })).unwrap();
    assert!(
        p.doc["scene"]["canvas"].get("ambient").is_none() && p.doc["scene"]["canvas"].get("sky").is_none()
    );
}
