use super::{SIZE, Shot, UP, at, files, renderer, scene};
use serde_json::{Value, json};
use wprs_scene::{Quality, Wake};

fn lamp(shafts: f32) -> Value {
    sized(shafts, 50)
}

fn sized(shafts: f32, radius: u32) -> Value {
    json!({ "id": "lamp", "type": "light", "transform": { "position": [80, 45] }, "z": 10, "radius": radius,
        "intensity": 0.4, "volumetric": { "density": 0.8, "samples": 16, "shafts": shafts } })
}

fn half() -> Value {
    json!({ "id": "half", "type": "image", "image": "mask", "size": [160, 90], "opacity": 0.3,
        "transform": { "position": [80, 45] } })
}

fn shot(layers: Value) -> Option<image::RgbaImage> {
    let r = renderer()?;
    Some(Shot::new(r, scene(layers, json!({})), &files(UP)).pixels(r, Quality::Q3))
}

fn glow(lit: &image::RgbaImage, below: &image::RgbaImage, x: u32) -> i32 {
    i32::from(at(lit, x, 45)[1]) - i32::from(at(below, x, 45)[1])
}

#[test]
fn layers_above_a_light_cover_its_air() {
    let wall = json!({ "id": "wall", "type": "image", "image": "wall", "size": [160, 90], "transform": { "position": [80, 45] } });
    let (Some(under), Some(over), Some(plain)) =
        (shot(json!([lamp(0.0), wall.clone()])), shot(json!([wall.clone(), lamp(0.0)])), shot(json!([wall])))
    else {
        return;
    };
    assert_eq!(at(&under, 80, 45), at(&plain, 80, 45), "the wall in front hides the air");
    assert!(at(&over, 80, 45)[0] > at(&plain, 80, 45)[0] + 20, "{:?}", at(&over, 80, 45));
}

#[test]
fn dark_parts_behind_a_light_hold_back_its_air_by_its_shafts() {
    let (Some(plain), Some(even), Some(cut)) =
        (shot(json!([half()])), shot(json!([half(), lamp(0.0)])), shot(json!([half(), lamp(1.0)])))
    else {
        return;
    };
    assert!(glow(&even, &plain, 64) > 60, "without shafts the air lights the dark half too");
    assert!(glow(&cut, &plain, 64) < 6, "the dark half holds back the air: {}", glow(&cut, &plain, 64));
    assert!(
        glow(&cut, &even, 96).abs() < 4 && glow(&cut, &plain, 96) > 20,
        "the bright half lets it through"
    );
}

#[test]
fn still_shafts_are_drawn_once_and_a_moving_layer_behind_redraws_the_air() {
    let Some(r) = renderer() else { return };
    let mut still = Shot::new(r, scene(json!([half(), lamp(1.0)]), json!({})), &files(UP));
    let first = still.pixels(r, Quality::Q3);
    still.runtime.advance(0.5);
    assert_eq!(still.runtime.frame(still.view.view(SIZE), Quality::Q3).wake, Wake::Idle);
    assert_eq!(still.pixels(r, Quality::Q3), first);
    assert_eq!(still.view.stats().draws, 0, "{:?}", still.view.stats());
    let spin = json!({ "timeline": "go", "keys": [[0, 0], [4, 360]] });
    let dot = json!({ "id": "dot", "type": "shape", "shape": { "rect": { "size": [6, 6] } }, "fill": "#ffffff",
        "transform": { "position": [70, 45], "rotation": spin } });
    let mut moving = Shot::new(r, scene(json!([half(), dot, sized(1.0, 30)]), json!({})), &files(UP));
    moving.pixels(r, Quality::Q3);
    moving.runtime.advance(0.1);
    moving.pixels(r, Quality::Q3);
    let region = moving.view.damage().unwrap();
    assert!(region.width >= 60 && region.width < 80, "the box of the light is redrawn: {region:?}");
}
