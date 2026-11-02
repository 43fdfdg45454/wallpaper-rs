use super::{SIZE, Shot, UP, files, renderer, scene, wall};
use serde_json::json;
use wprs_scene::{Quality, Wake};

fn moving(quality: Quality) -> Option<(Shot, &'static wprs_render::Renderer)> {
    let r = renderer()?;
    let walk = json!({ "timeline": "go", "keys": [[0, [30, 45]], [4, [130, 45]]] });
    let lamp =
        json!({ "id": "lamp", "type": "light", "transform": { "position": walk }, "z": 10, "radius": 12 });
    let mut shot = Shot::new(r, scene(json!([wall(json!({})), lamp]), json!({})), &files(UP));
    shot.pixels(r, quality);
    Some((shot, r))
}

#[test]
fn a_moving_light_redraws_only_the_box_of_its_radius() {
    let Some((mut shot, r)) = moving(Quality::Q3) else { return };
    shot.runtime.advance(0.1);
    shot.pixels(r, Quality::Q3);
    let damage = shot.view.stats().damage;
    assert!(damage > 0 && damage < 10, "{damage} % of the canvas");
    let region = shot.view.damage().unwrap();
    assert!(region.width < 60 && region.height < 40, "{region:?}");
}

#[test]
fn static_lights_keep_the_lit_layers_drawn_once() {
    let Some(r) = renderer() else { return };
    let lamp = json!({ "id": "lamp", "type": "light", "transform": { "position": [40, 45] }, "z": 10, "radius": 60 });
    let spin = json!({ "timeline": "go", "keys": [[0, 0], [4, 360]] });
    let dot = json!({ "id": "dot", "type": "shape", "shape": { "rect": { "size": [30, 30] } }, "fill": "#ff0000",
        "transform": { "position": [80, 45], "rotation": spin } });
    let mut shot = Shot::new(r, scene(json!([wall(json!({})), lamp, dot]), json!({})), &files(UP));
    let first = shot.pixels(r, Quality::Q3);
    for _ in 0..3 {
        shot.runtime.advance(0.1);
        shot.pixels(r, Quality::Q3);
        assert!(shot.view.stats().reused > 0, "{:?}", shot.view.stats());
    }
    assert_eq!(first.get_pixel(40, 45), shot.pixels(r, Quality::Q3).get_pixel(40, 45));
}

#[test]
fn the_lowest_level_bakes_the_lights_as_they_start() {
    let Some((mut shot, r)) = moving(Quality::Q0) else { return };
    let start = shot.pixels(r, Quality::Q0);
    shot.runtime.advance(1.0);
    let later = shot.pixels(r, Quality::Q0);
    assert_eq!(start.get_pixel(30, 45), later.get_pixel(30, 45));
    assert_eq!(shot.view.stats().draws, 0, "{:?}", shot.view.stats());
}

#[test]
fn static_lights_let_the_scene_settle() {
    let Some(r) = renderer() else { return };
    let lamp = json!({ "id": "lamp", "type": "light", "transform": { "position": [40, 45] }, "radius": 60 });
    let mut shot =
        Shot::new(r, scene(json!([wall(json!({})), lamp]), json!({ "ambient": "#202020" })), &files(UP));
    for q in [Quality::Q3, Quality::Q0] {
        shot.pixels(r, q);
        assert_eq!(shot.runtime.frame(shot.view.view(SIZE), q).wake, Wake::Idle);
    }
}

#[test]
fn a_moving_light_asks_for_frames_except_when_baked() {
    let Some((shot, _)) = moving(Quality::Q3) else { return };
    let view = shot.view.view(SIZE);
    let lights_only = |q| shot.runtime.frame(view, q).wake;
    assert_eq!(lights_only(Quality::Q3), Wake::Frame);
    assert_eq!(lights_only(Quality::Q0), Wake::Idle);
}
