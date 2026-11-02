use super::{Shot, UP, files, renderer, scene, wall};
use serde_json::json;
use std::sync::Arc;
use wprs_scene::{Quality, Runtime, Values};

fn lamp(x: f32) -> serde_json::Value {
    json!({ "id": "lamp", "type": "light", "transform": { "position": [x, 45] }, "z": 10, "radius": 30 })
}

fn row(image: &image::RgbaImage, y: u32) -> Vec<u8> {
    (0..image.width()).step_by(8).map(|x| image.get_pixel(x, y).0[0]).collect()
}

#[test]
fn the_lowest_level_bakes_the_lights_again_for_a_new_size() {
    let Some(r) = renderer() else { return };
    let lit = || scene(json!([wall(json!({})), lamp(40.0)]), json!({}));
    let mut shot = Shot::new(r, lit(), &files(UP));
    shot.pixels(r, Quality::Q0);
    let resized = shot.sized(r, Quality::Q0, (320, 180));
    let fresh = Shot::new(r, lit(), &files(UP)).sized(r, Quality::Q0, (320, 180));
    assert_eq!(row(&resized, 90), row(&fresh, 90));
    assert!(resized.get_pixel(80, 90).0[0] > 100, "{:?}", resized.get_pixel(80, 90));
}

#[test]
fn the_lowest_level_bakes_the_lights_again_when_the_scene_is_refreshed() {
    let Some(r) = renderer() else { return };
    let at = |x| scene(json!([wall(json!({})), lamp(x)]), json!({}));
    let mut shot = Shot::new(r, at(30.0), &files(UP));
    shot.pixels(r, Quality::Q0);
    let moved = at(130.0);
    r.refresh_scene(&mut shot.view, (&moved, &Values::new()), &files(UP)).unwrap();
    shot.runtime = Runtime::new(Arc::new(moved), Values::new());
    let after = shot.pixels(r, Quality::Q0);
    let fresh = Shot::new(r, at(130.0), &files(UP)).pixels(r, Quality::Q0);
    assert_eq!(row(&after, 45), row(&fresh, 45));
    assert!(after.get_pixel(130, 45).0[0] > after.get_pixel(30, 45).0[0] + 50, "{:?}", row(&after, 45));
}

#[test]
fn the_lowest_level_bakes_the_lights_again_after_a_retry() {
    let Some(r) = renderer() else { return };
    let mut shot = Shot::new(r, scene(json!([wall(json!({})), lamp(40.0)]), json!({})), &files(UP));
    let first = shot.pixels(r, Quality::Q0);
    shot.view.retry();
    assert_eq!(shot.pixels(r, Quality::Q0), first);
}
