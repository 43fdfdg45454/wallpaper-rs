use super::{SIZE, Shot, UP, at, files, renderer, scene};
use serde_json::{Value, json};
use wprs_scene::{Quality, Wake};

fn lamp(more: Value) -> Value {
    let mut l = json!({ "id": "lamp", "type": "light", "transform": { "position": [80, 45] }, "z": 10,
        "radius": 40, "intensity": 2, "volumetric": { "density": 0.8, "samples": 16 } });
    more.as_object().into_iter().flatten().for_each(|(k, v)| l[k] = v.clone());
    l
}

fn shot(light: Value, quality: Quality) -> Option<image::RgbaImage> {
    let r = renderer()?;
    Some(Shot::new(r, scene(json!([light]), json!({})), &files(UP)).pixels(r, quality))
}

#[test]
fn a_volumetric_light_lights_the_air_inside_its_radius() {
    let Some(lit) = shot(lamp(json!({})), Quality::Q3) else { return };
    let Some(plain) = shot(lamp(json!({ "volumetric": null })), Quality::Q3) else { return };
    assert!(at(&lit, 80, 45)[0] > 40, "{:?}", at(&lit, 80, 45));
    assert!(at(&lit, 80, 45)[0] > at(&lit, 110, 45)[0], "it fades out");
    assert_eq!(at(&lit, 140, 45), [0, 0, 0, 255], "nothing past the radius");
    assert_eq!(at(&plain, 80, 45), [0, 0, 0, 255], "a light without air draws nothing");
}

#[test]
fn volumetrics_are_off_at_the_lowest_level_and_coarser_below_the_top() {
    let Some(low) = shot(lamp(json!({})), Quality::Q0) else { return };
    assert_eq!(at(&low, 80, 45), [0, 0, 0, 255]);
    let (Some(q1), Some(q3)) = (shot(lamp(json!({})), Quality::Q1), shot(lamp(json!({})), Quality::Q3))
    else {
        return;
    };
    assert!(at(&q1, 80, 45)[0] > 20, "{:?}", at(&q1, 80, 45));
    assert!(at(&q1, 80, 45)[0].abs_diff(at(&q3, 80, 45)[0]) < 60);
    let none = shot(lamp(json!({ "volumetric": { "density": 0, "samples": 4 } })), Quality::Q3);
    assert_eq!(none.map(|n| at(&n, 80, 45)), Some([0, 0, 0, 255]));
}

#[test]
fn a_spot_lights_the_air_along_its_cone() {
    let spot = lamp(json!({ "light": "spot", "cone": [20, 50], "radius": 70,
        "transform": { "position": [80, 10], "tilt": [30, 0] } }));
    let Some(beam) = shot(spot, Quality::Q3) else { return };
    assert!(
        at(&beam, 80, 40)[0] > at(&beam, 30, 12)[0] + 10,
        "{:?} {:?}",
        at(&beam, 80, 40),
        at(&beam, 30, 12)
    );
}

#[test]
fn still_volumetric_lights_settle_and_a_moving_one_redraws_its_box() {
    let Some(r) = renderer() else { return };
    let mut still = Shot::new(r, scene(json!([lamp(json!({}))]), json!({})), &files(UP));
    let first = still.pixels(r, Quality::Q3);
    still.runtime.advance(0.5);
    assert_eq!(still.runtime.frame(still.view.view(SIZE), Quality::Q3).wake, Wake::Idle);
    assert_eq!(still.pixels(r, Quality::Q3), first);
    assert_eq!(still.view.stats().draws, 0, "{:?}", still.view.stats());
    let walk = json!({ "timeline": "go", "keys": [[0, [30, 45]], [4, [130, 45]]] });
    let moving = lamp(json!({ "radius": 12, "transform": { "position": walk } }));
    let mut shot = Shot::new(r, scene(json!([moving]), json!({})), &files(UP));
    shot.pixels(r, Quality::Q3);
    shot.runtime.advance(0.1);
    shot.pixels(r, Quality::Q3);
    let damage = shot.view.stats().damage;
    assert!(damage > 0 && damage < 10, "{damage} % of the canvas");
}
