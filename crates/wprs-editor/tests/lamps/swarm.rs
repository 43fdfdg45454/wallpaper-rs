use super::{act, layer, project, undone};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

fn render(p: &wprs_editor::Project) -> Value {
    p.doc["scene"]["particles"]["flakes"]["render"].clone()
}

fn with_bump(p: &mut wprs_editor::Project) {
    RgbaImage::from_pixel(8, 8, Rgba([255, 128, 128, 255])).save(p.dir.join("bump.png")).unwrap();
    let value = json!({ "path": "bump.png", "linear": true });
    act(p, json!({ "set": { "pointer": "/scene/assets/bump", "value": value } })).unwrap();
}

#[test]
fn particles_take_light_with_a_normal_map_and_undo() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    with_bump(&mut p);
    let before = p.doc.clone();
    act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "lit": true, "normal": "bump" } } }))
        .unwrap();
    undone(&mut p, &before);
    assert_eq!((render(&p)["lit"].clone(), render(&p)["normal"].clone()), (json!(true), json!("bump")));
    act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "normal": "" } } })).unwrap();
    assert!(render(&p).get("normal").is_none());
    assert!(
        act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "normal": "song" } } })).is_err()
    );
    assert!(act(&mut p, json!({ "particles": { "lit": { "system": "nothing", "lit": true } } })).is_err());
    act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "layer": "flakes", "lit": false } } }))
        .unwrap();
    assert_eq!(layer(&p, "flakes")["render"].get("lit"), None, "the layer keeps its own look");
    act(&mut p, json!({ "particles": { "lit": { "system": "flakes", "lit": false } } })).unwrap();
    assert!(render(&p).get("lit").is_none());
}

#[test]
fn refraction_needs_a_normal_map_and_an_amount_in_range() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    with_bump(&mut p);
    let bend = |args: Value| json!({ "particles": { "refract": args } });
    assert!(act(&mut p, bend(json!({ "system": "flakes", "amount": 0.5 }))).is_err(), "no normal map yet");
    let before = p.doc.clone();
    act(&mut p, bend(json!({ "system": "flakes", "normal": "bump", "amount": 0.5 }))).unwrap();
    undone(&mut p, &before);
    assert_eq!(render(&p)["refract"], json!({ "normal": "bump", "amount": 0.5 }));
    act(&mut p, bend(json!({ "system": "flakes", "amount": 0.2 }))).unwrap();
    assert_eq!(render(&p)["refract"]["amount"], 0.2);
    assert!(act(&mut p, bend(json!({ "system": "flakes", "amount": 1.5 }))).is_err());
    assert!(act(&mut p, bend(json!({ "system": "flakes", "normal": "song" }))).is_err());
    let shown = wprs_editor::lamps::swarm_light(&p.dir, &p.doc, "flakes", None);
    assert!(shown["field"].as_str().unwrap().starts_with("data:image/png"));
    assert!((shown["shift"].as_f64().unwrap() - 0.2 * 0.5 * 6.0).abs() < 1e-6, "{shown}");
    let levels = shown["levels"].as_array().unwrap();
    assert_eq!((levels[0]["refract"].clone(), levels[1]["refract"].clone()), (Value::Null, Value::Null));
    assert_eq!((levels[2]["refract"].as_f64(), levels[3]["refract"].as_f64()), (Some(0.5), Some(1.0)));
    let cost = wprs_editor::lamps::cost(&p.dir, &p.doc, 0.0, wprs_scene::Quality::Q3);
    assert_eq!(cost["refracting"], json!(["flakes"]));
    assert!(
        cost["levels"][1]["refract_pixels"] == 0.0
            && cost["levels"][3]["refract_pixels"].as_f64() > Some(0.0)
    );
    act(&mut p, bend(json!({ "system": "flakes", "off": true }))).unwrap();
    assert!(render(&p).get("refract").is_none());
}

#[test]
fn the_two_sided_preview_lights_the_front_and_the_back() {
    let flat = wprs_editor::lamps::sides(None);
    assert_eq!((flat.width(), flat.height()), (96, 48));
    let (front, back) = (flat.get_pixel(24, 24).0, flat.get_pixel(72, 24).0);
    assert!(front[0] > 100 && front == back, "{front:?} {back:?}");
    let turned = RgbaImage::from_pixel(48, 48, Rgba([51, 204, 195, 255]));
    let side = wprs_editor::lamps::sides(Some(&turned));
    assert!(
        side.get_pixel(24, 24).0[0] > flat.get_pixel(24, 24).0[0],
        "a normal turned to the light is brighter"
    );
    assert_eq!(side.get_pixel(0, 0).0[3], 0, "outside the disc is clear");
}
