use super::{UP, at, once, wall};
use serde_json::json;

fn lamp(light: serde_json::Value) -> serde_json::Value {
    let mut l = json!({ "id": "lamp", "type": "light", "transform": { "position": [40, 45] }, "z": 20, "radius": 60 });
    l.as_object_mut().unwrap().extend(light.as_object().cloned().unwrap());
    l
}

#[test]
fn a_point_light_falls_off_to_nothing_at_its_radius() {
    let Some(img) = once(json!([wall(json!({})), lamp(json!({}))]), json!({}), UP) else { return };
    let (near, mid, far) = (at(&img, 40, 45)[0], at(&img, 70, 45)[0], at(&img, 110, 45)[0]);
    assert!(near > 150 && mid > 10 && mid < near && far == 0, "{near} {mid} {far}");
}

#[test]
fn a_spot_light_lights_only_inside_its_cone() {
    let spot = lamp(
        json!({ "light": "spot", "cone": [20, 40], "transform": { "position": [80, 10], "tilt": [60, 0] } }),
    );
    let Some(img) = once(json!([wall(json!({})), spot]), json!({}), UP) else { return };
    let (inside, outside) = (at(&img, 80, 30)[0], at(&img, 40, 30)[0]);
    assert!(inside > 40 && outside < inside / 4, "{inside} {outside}");
}

#[test]
fn a_tube_light_lights_along_its_length() {
    let tube =
        lamp(json!({ "light": "tube", "length": 100, "radius": 30, "transform": { "position": [80, 45] } }));
    let Some(img) = once(json!([wall(json!({})), tube]), json!({}), UP) else { return };
    let (end, middle, above) = (at(&img, 40, 45)[0], at(&img, 80, 45)[0], at(&img, 80, 10)[0]);
    assert!(end > 100 && middle > 100 && above == 0, "{end} {middle} {above}");
}

#[test]
fn a_directional_light_reaches_everywhere() {
    let sun = lamp(json!({ "light": "directional", "intensity": 0.5, "transform": { "tilt": [89, 0] } }));
    let Some(img) = once(json!([wall(json!({})), sun]), json!({}), UP) else { return };
    let (a, b) = (at(&img, 5, 5)[0], at(&img, 150, 80)[0]);
    assert!(a > 60 && a.abs_diff(b) <= 1, "{a} {b}");
}

#[test]
fn a_layer_without_lighting_ignores_lights() {
    let plain = json!({ "id": "wall", "type": "image", "image": "wall", "size": [160, 90], "transform": { "position": [80, 45] } });
    let Some(img) = once(json!([plain, lamp(json!({}))]), json!({}), UP) else { return };
    assert_eq!(at(&img, 40, 45), at(&img, 140, 80));
}

#[test]
fn modest_devices_light_with_at_most_eight_lamps() {
    let Some(r) = super::renderer() else { return };
    let lamps: Vec<_> = (0..10)
        .map(|i| {
            json!({ "id": format!("l{i}"), "type": "light", "transform": { "position": [8 + 16 * i, 45] },
            "z": 4, "radius": 6 })
        })
        .collect();
    let mut layers = vec![wall(json!({}))];
    layers.extend(lamps);
    let Some(img) = once(json!(layers), json!({}), UP) else { return };
    let lit = (0..10).filter(|i| at(&img, 8 + 16 * i, 45)[0] > 50).count();
    let expected = if r.tier() <= wprs_scene::Tier::T1 { 8 } else { 10 };
    assert_eq!(lit, expected);
}

#[test]
fn texts_take_the_light_too() {
    let text = json!({ "id": "t", "type": "text", "text": "████", "size": 40, "color": "#ffffff",
        "transform": { "position": [80, 45] }, "lighting": {} });
    let Some(dark) = once(json!([text.clone()]), json!({}), UP) else { return };
    let Some(lit) = once(
        json!([text, lamp(json!({ "radius": 200, "transform": { "position": [80, 45] } }))]),
        json!({}),
        UP,
    ) else {
        return;
    };
    let bright = |img: &image::RgbaImage| img.pixels().map(|p| u32::from(p.0[0])).max().unwrap_or(0);
    assert!(bright(&dark) < 5 && bright(&lit) > 100, "{} {}", bright(&dark), bright(&lit));
}
