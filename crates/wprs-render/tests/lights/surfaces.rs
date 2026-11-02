use super::{UP, at, once, wall};
use serde_json::json;

fn left(z: f32) -> serde_json::Value {
    json!({ "id": "lamp", "type": "light", "transform": { "position": [0, 45] }, "z": z, "radius": 400 })
}

#[test]
fn a_normal_map_turns_the_surface_towards_the_light() {
    let lit = json!({ "normal": "bump" });
    let facing = once(json!([wall(lit.clone()), left(10.0)]), json!({}), [10, 128, 140, 255]);
    let away = once(json!([wall(lit), left(10.0)]), json!({}), [245, 128, 140, 255]);
    let (Some(facing), Some(away)) = (facing, away) else { return };
    let (a, b) = (at(&facing, 80, 45)[0], at(&away, 80, 45)[0]);
    assert!(a > b + 30, "{a} {b}");
}

#[test]
fn smooth_surfaces_shine_and_rough_ones_do_not() {
    let light = json!({ "id": "lamp", "type": "light", "transform": { "position": [80, 45] }, "z": 60, "radius": 400 });
    let gloss = once(json!([wall(json!({ "roughness": 0.0 })), light.clone()]), json!({}), UP);
    let matte = once(json!([wall(json!({ "roughness": 1.0 })), light]), json!({}), UP);
    let (Some(gloss), Some(matte)) = (gloss, matte) else { return };
    assert!(at(&gloss, 80, 45)[0] > at(&matte, 80, 45)[0] + 20);
}

#[test]
fn emissive_layers_glow_in_the_dark() {
    let Some(img) = once(json!([wall(json!({ "emissive": 0.5 }))]), json!({}), UP) else { return };
    let v = at(&img, 80, 45)[0];
    assert!((90..=150).contains(&v), "{v}");
}

#[test]
fn an_emissive_map_limits_the_glow_to_its_white_part() {
    let Some(img) = once(json!([wall(json!({ "emissive": 1.0, "emissive_map": "mask" }))]), json!({}), UP)
    else {
        return;
    };
    assert!(at(&img, 20, 45)[0] < 5 && at(&img, 140, 45)[0] > 150);
}

#[test]
fn a_reflection_mixes_in_its_picture() {
    let Some(img) =
        once(json!([wall(json!({ "reflection": 1.0, "roughness": 0.0, "env": "env" }))]), json!({}), UP)
    else {
        return;
    };
    let [b, _, r, _] = at(&img, 80, 45);
    assert!(b > 200 && r < 30, "{r} {b}");
}

#[test]
fn ambient_and_sky_light_the_layer_by_its_normal() {
    let canvas = json!({ "ambient": "#404040", "sky": "#ffffff" });
    let flat = once(json!([wall(json!({}))]), canvas.clone(), UP);
    let up = once(json!([wall(json!({ "normal": "bump" }))]), canvas, [128, 255, 128, 255]);
    let (Some(flat), Some(up)) = (flat, up) else { return };
    let (a, b) = (at(&flat, 80, 45)[0], at(&up, 80, 45)[0]);
    assert!(b > a + 20 && a > 60, "{a} {b}");
}

#[test]
fn strength_mixes_the_lit_and_plain_layer() {
    let Some(img) = once(json!([wall(json!({ "strength": 0.0 }))]), json!({}), UP) else { return };
    let v = at(&img, 80, 45)[0];
    assert!(v > 190, "{v}");
}
