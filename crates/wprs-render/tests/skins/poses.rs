use super::kit::{BLUE, ORANGE, RED, SIDE, mesh, near, picture, render, renderer, rgb, scene, sketch};
use serde_json::json;
use wprs_scene::Quality;

#[test]
fn the_rest_pose_draws_exactly_the_plain_picture() {
    let Some(r) = renderer() else { return };
    let plain = json!({ "id": "fox", "type": "image", "image": "pic", "size": [SIDE, SIDE] });
    let flat = render(r, scene(plain, sketch(false)), 0.0, Quality::Q3);
    let bent = render(r, scene(mesh(json!({})), sketch(false)), 0.0, Quality::Q3);
    let worst = flat.iter().zip(&bent).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 2, "the mesh differs from the picture by {worst}");
    assert!(
        near(rgb(&bent, 56, 32), RED) && near(rgb(&bent, 4, 4), BLUE) && near(rgb(&bent, 30, 50), ORANGE)
    );
    let _ = picture();
}

#[test]
fn a_bent_bone_swings_its_part_of_the_picture() {
    let Some(r) = renderer() else { return };
    let bend = json!({ "clips": [{ "id": "bend", "bones": { "arm": { "rotation": [[0, 90]] } } }],
        "animations": [{ "clip": "bend" }] });
    let px = render(r, scene(mesh(bend), sketch(false)), 0.0, Quality::Q3);
    assert!(!near(rgb(&px, 58, 32), RED), "{:?}", rgb(&px, 58, 32));
    assert!(near(rgb(&px, 32, 58), RED), "{:?}", rgb(&px, 32, 58));
    assert!(near(rgb(&px, 4, 4), BLUE));
}

#[test]
fn a_clip_draws_its_pose_at_a_given_time() {
    let Some(r) = renderer() else { return };
    let turn = json!({ "clips": [{ "id": "turn", "loop": "once", "bones": { "arm": { "rotation": [[0, 0], [1, 90]] } } }],
        "animations": [{ "clip": "turn" }] });
    let px = render(r, scene(mesh(turn), sketch(false)), 0.5, Quality::Q3);
    assert!(near(rgb(&px, 50, 50), RED), "{:?}", rgb(&px, 50, 50));
    assert!(!near(rgb(&px, 60, 4), RED), "{:?}", rgb(&px, 60, 4));
    let low = render(r, scene(mesh(json!({})), sketch(false)), 0.0, Quality::Q0);
    assert!(near(rgb(&low, 58, 32), RED) && near(rgb(&low, 30, 50), ORANGE));
}

#[test]
fn blend_shapes_move_the_picture_by_their_weight() {
    let Some(r) = renderer() else { return };
    let rest = render(r, scene(mesh(json!({})), sketch(true)), 0.0, Quality::Q3);
    assert!(near(rgb(&rest, 4, 20), ORANGE));
    let full = render(r, scene(mesh(json!({ "shapes": { "up": 1.0 } })), sketch(true)), 0.0, Quality::Q3);
    assert!(
        near(rgb(&full, 30, 2), ORANGE) && near(rgb(&full, 30, 60), [0, 0, 0]),
        "{:?}",
        rgb(&full, 30, 60)
    );
    let half = render(r, scene(mesh(json!({ "shapes": { "up": 0.5 } })), sketch(true)), 0.0, Quality::Q3);
    assert!(
        near(rgb(&half, 4, 4), BLUE) && !near(rgb(&half, 4, 10), BLUE) && near(rgb(&half, 30, 60), [0, 0, 0])
    );
}
