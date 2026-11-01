use super::kit::{act, info, query, rigged};
use serde_json::{Value, json};
use wprs_editor::Project;
use wprs_editor::rigs::{heat_color, hue, vertices};

fn listed(p: &Project) -> Vec<Value> {
    vertices(&p.dir, &p.doc, "net").as_array().unwrap().clone()
}

fn weight(v: &Value, bone: &str) -> f64 {
    v["weights"][bone].as_f64().unwrap_or(0.0)
}

fn whole(p: &Project) {
    for v in listed(p) {
        let w = v["weights"].as_object().unwrap();
        let sum: f64 = w.values().map(|x| x.as_f64().unwrap()).sum();
        assert!(w.len() <= 4 && (sum - 1.0).abs() < 1e-3, "{v}");
    }
}

fn brush(bone: &str, mode: &str, by: f32, radius: f32, lock: &[&str]) -> Value {
    json!({ "weights": { "layer": "net", "bone": bone, "points": [[0.125, 0.5]], "mode": mode, "by": by,
        "radius": radius, "falloff": 0, "lock": lock } })
}

fn at(p: &Project, x: f64) -> Value {
    listed(p)
        .into_iter()
        .find(|v| (v["at"][0].as_f64().unwrap() - x).abs() < 1e-3 && v["at"][1] == 0.5)
        .unwrap()
}

#[test]
fn a_brush_paints_the_weights_of_a_bone_and_keeps_them_whole() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    assert_eq!(info(&p, "net")["auto"], json!(true));
    let far = at(&p, 0.875);
    act(&mut p, brush("arm", "add", 1.0, 0.05, &[])).unwrap();
    assert_eq!((weight(&at(&p, 0.125), "arm"), weight(&at(&p, 0.125), "root")), (1.0, 0.0));
    assert_eq!(at(&p, 0.875), far);
    assert_eq!(info(&p, "net")["auto"], json!(false));
    whole(&p);
    act(&mut p, brush("arm", "subtract", 0.25, 0.05, &[])).unwrap();
    assert_eq!((weight(&at(&p, 0.125), "arm"), weight(&at(&p, 0.125), "root")), (0.75, 0.25));
    act(&mut p, brush("arm", "subtract", 0.25, 0.05, &["root"])).unwrap();
    assert_eq!(weight(&at(&p, 0.125), "root"), 0.25);
    act(&mut p, brush("arm", "smooth", 1.0, 0.5, &[])).unwrap();
    whole(&p);
    act(&mut p, brush("arm", "normalise", 0.3, 1.0, &[])).unwrap();
    whole(&p);
    assert!(
        listed(&p).iter().all(|v| v["weights"]
            .as_object()
            .unwrap()
            .values()
            .all(|w| w.as_f64().unwrap() >= 0.3))
    );
    let painted = listed(&p);
    act(&mut p, json!({ "undo": {} })).unwrap();
    assert_ne!(listed(&p), painted);
    act(&mut p, json!({ "redo": {} })).unwrap();
    assert_eq!(listed(&p), painted);
    for bad in [
        brush("tail", "add", 1.0, 0.1, &[]),
        brush("arm", "add", 1.0, 0.1, &["arm"]),
        brush("arm", "add", 2.0, 0.1, &[]),
        brush("arm", "add", 1.0, 0.0, &[]),
        json!({ "weights": { "layer": "net", "bone": "arm", "points": [], "mode": "add", "by": 1 } }),
        json!({ "weights": { "layer": "nothing", "bone": "arm", "points": [[0.5, 0.5]], "mode": "add", "by": 1 } }),
    ] {
        assert!(act(&mut p, bad).is_err());
    }
}

#[test]
fn painted_weights_stay_when_bones_change_and_auto_weights_bring_them_back() {
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, brush("arm", "add", 1.0, 0.05, &[])).unwrap();
    act(&mut p, json!({ "bone": { "layer": "net", "name": "hand", "parent": "arm", "at": [0.9, 0.5] } }))
        .unwrap();
    assert_eq!(weight(&at(&p, 0.125), "arm"), 1.0);
    assert!(listed(&p).iter().all(|v| weight(v, "hand") == 0.0));
    act(&mut p, json!({ "bone": { "layer": "net", "name": "arm", "remove": true } })).unwrap();
    assert_eq!(weight(&at(&p, 0.125), "root"), 1.0);
    whole(&p);
    act(&mut p, json!({ "auto_weights": { "layer": "net" } })).unwrap();
    assert_eq!(info(&p, "net")["auto"], json!(true));
    assert!(weight(&at(&p, 0.875), "hand") > 0.2);
    assert!(weight(&at(&p, 0.125), "root") > 0.5);
    assert!(act(&mut p, json!({ "auto_weights": { "layer": "nothing" } })).is_err());
}

#[test]
fn the_heat_view_colours_triangles_by_the_weight_of_the_picked_bone() {
    assert_eq!(
        (heat_color(0.0), heat_color(0.5), heat_color(1.0)),
        ("#1e2878".into(), "#32b450".into(), "#dc2828".into())
    );
    assert_eq!(heat_color(0.125), "#1e5aa5");
    assert_ne!(hue(0), hue(1));
    let root = tempfile::tempdir().unwrap();
    let mut p = rigged(&root);
    act(&mut p, brush("arm", "add", 1.0, 1.0, &[])).unwrap();
    let shown = query(&p, "net", json!({ "weights": "arm" }));
    let (heat, owner) = (picture(&shown["heat"]), picture(&shown["owner"]));
    assert_eq!(shown["size"], json!([heat.width(), heat.height()]));
    assert_eq!((heat.width(), heat.height()), (384, 192));
    assert_eq!(heat.get_pixel(192, 96).0, [0xdc, 0x28, 0x28, 255]);
    let covered =
        |i: &image::RgbaImage| i.pixels().filter(|q| q.0[3] == 255).map(|q| hex(q.0)).collect::<Vec<_>>();
    assert!(covered(&heat).len() > 384 * 150 && covered(&heat).iter().all(|c| c == "#dc2828"));
    assert!(covered(&owner).iter().all(|c| *c == hue(1)));
    assert_eq!((shown["bones"].clone(), shown["colors"][1].clone()), (json!(["root", "arm"]), json!(hue(1))));
    let cold = query(&p, "net", json!({ "weights": "root" }));
    assert!(covered(&picture(&cold["heat"])).iter().all(|c| c == "#1e2878"));
    act(&mut p, brush("root", "add", 1.0, 0.5, &[])).unwrap();
    let mixed = picture(&query(&p, "net", json!({ "weights": "arm" }))["heat"]);
    let shades: std::collections::BTreeSet<String> = covered(&mixed).into_iter().collect();
    assert!(shades.len() > 2, "{shades:?}");
}

fn picture(url: &Value) -> image::RgbaImage {
    use base64::Engine;
    let text = url.as_str().unwrap().strip_prefix("data:image/png;base64,").unwrap();
    let png = base64::engine::general_purpose::STANDARD.decode(text).unwrap();
    image::load_from_memory(&png).unwrap().into_rgba8()
}

fn hex([r, g, b, _]: [u8; 4]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}
