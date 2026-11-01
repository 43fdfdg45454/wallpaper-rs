use super::*;
use wprs_scene::Wake;

mod bloom;
mod capture;
mod films;
mod tilt;

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];

fn halved(r: &Renderer, mut canvas: Value, more: Value) -> Shot {
    let mut layers = vec![json!({ "id": "img", "type": "image", "image": "rb", "size": [100, 50] })];
    layers.extend(more.as_array().cloned().unwrap_or_default());
    canvas["background"] = json!("#000000");
    canvas["size"] = json!([100, 50]);
    Shot::new(r, canvas, json!(layers), &[("rb", halves([255, 0, 0, 255], [0, 0, 255, 255]))])
}

pub(crate) fn at_level(shot: &mut Shot, r: &Renderer, quality: Quality) -> Vec<u8> {
    let frame = shot.runtime.frame(shot.view.view(SIZE), quality);
    let image = r.render_scene(&mut shot.view, &shot.runtime, &frame, SIZE, quality, &[]).unwrap();
    let mut target = r.create_target(SIZE.0, SIZE.1).unwrap();
    let layer = Layer {
        image: Some(image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
    };
    let mut pixels = Vec::new();
    r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
    pixels
}

#[test]
fn the_camera_zooms_turns_and_moves_the_whole_scene() {
    let Some(r) = renderer() else { return };
    let zoom = halved(r, json!({ "camera": { "center": [25, 25], "zoom": 2 } }), json!([])).pixels(r);
    check(&zoom, &[((5, 5), RED), ((95, 45), RED), ((50, 25), RED)]);
    let turned = halved(r, json!({ "camera": { "rotation": 180 } }), json!([])).pixels(r);
    check(&turned, &[((10, 25), BLUE), ((90, 25), RED)]);
    let moved = halved(r, json!({ "camera": { "center": [70, 25] } }), json!([])).pixels(r);
    check(&moved, &[((25, 25), RED), ((35, 25), BLUE)]);
    let plain = halved(r, json!({}), json!([])).pixels(r);
    assert_eq!(halved(r, json!({ "camera": {} }), json!([])).pixels(r), plain);
}

#[test]
fn camera_shake_runs_only_with_motion_and_above_the_lowest_level() {
    let Some(r) = renderer() else { return };
    let shaky = json!({ "camera": { "shake": { "amplitude": 10, "speed": 3 } } });
    let mut shot = halved(r, shaky.clone(), json!([]));
    shot.runtime.seek(0.7);
    let view = shot.view.view(SIZE);
    assert_eq!(shot.runtime.frame(view, Quality::Q3).wake, Wake::Frame);
    assert_eq!(shot.runtime.frame(view, Quality::Q0).wake, Wake::Idle);
    let moved = shot.pixels(r);
    let border = (0..100).find(|&x| at(&moved, x, 25)[2] > 128).unwrap();
    assert_ne!(border, 50, "the shake moves the scene");
    let mut still = halved(r, json!({}), json!([]));
    assert_eq!(at_level(&mut shot, r, Quality::Q0), at_level(&mut still, r, Quality::Q0));
    shot.runtime.set_reduced_motion(true);
    assert_eq!(shot.runtime.frame(view, Quality::Q3).wake, Wake::Idle);
    assert_eq!(shot.pixels(r), still.pixels(r));
    let off = json!({ "camera": { "shake": { "enabled": false, "amplitude": 10 } } });
    assert_eq!(halved(r, off, json!([])).runtime.frame(view, Quality::Q3).wake, Wake::Idle);
}

#[test]
fn a_canvas_that_does_not_clear_keeps_the_previous_frame() {
    let Some(r) = renderer() else { return };
    let mover = json!([{ "id": "m", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "transform": { "position": { "keys": [[0, [20, 25]], [1, [80, 25]]] } } }]);
    let mut kept = Shot::new(
        r,
        json!({ "size": [100, 50], "background": "#000000", "clear": false }),
        mover.clone(),
        &[],
    );
    let mut cleared = Shot::new(r, canvas("#000000"), mover, &[]);
    for shot in [&mut kept, &mut cleared] {
        shot.pixels(r);
        shot.runtime.seek(1.0);
    }
    check(&kept.pixels(r), &[((20, 25), [255; 3]), ((80, 25), [255; 3]), ((50, 5), [0; 3])]);
    check(&cleared.pixels(r), &[((20, 25), [0; 3]), ((80, 25), [255; 3])]);
    assert_eq!(kept.view.stats().damage, 100, "a kept frame is drawn whole");
    kept.pixels(r);
    assert_eq!(kept.view.stats().damage, 0, "nothing changed, so nothing is drawn");
}
