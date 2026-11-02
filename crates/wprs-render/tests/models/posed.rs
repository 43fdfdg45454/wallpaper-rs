use super::*;
use wprs_scene::{ForgeRig, Wake};

fn rig() -> Vec<u8> {
    ForgeRig { material: unlit("body", [1.0; 4]), ..ForgeRig::new(2.0, 0.5) }.glb()
}

fn lit(px: &[u8], x: u32, y: u32) -> bool {
    rgb(px, x, y)[0] > 128
}

fn model(extra: Value) -> Value {
    let mut layer = json!({ "id": "rig", "type": "model", "model": "rig" });
    extra.as_object().unwrap().iter().for_each(|(k, v)| layer[k] = v.clone());
    json!([layer])
}

fn posed(r: &Renderer, extra: Value, times: &[f64]) -> (Vec<Vec<u8>>, Shot) {
    let models = BTreeMap::from([("rig", rig())]);
    let mut shot = shoot(r, scene(json!({}), model(extra), models), &[]);
    let shots = times.iter().map(|dt| shot.frame(r, *dt, Quality::Q3)).collect();
    (shots, shot)
}

#[test]
fn a_clip_of_the_file_bends_the_skinned_column_on_the_gpu() {
    let Some(r) = renderer() else { return };
    let play =
        json!({ "clips": [{ "id": "bend", "animation": "bend" }], "animations": [{ "clip": "bend" }] });
    let (shots, _) = posed(r, play, &[0.0, 1.0]);
    assert!(lit(&shots[0], 32, 13), "upright at the start");
    assert!(!lit(&shots[1], 32, 12), "the tip leaves the middle");
    assert!((20..30).any(|x| lit(&shots[1], x, 14)), "and leans to the left");
    assert!(lit(&shots[1], 32, 28), "the root stays");
}

#[test]
fn a_still_skinned_model_without_clips_draws_its_rest_pose_like_a_plain_one() {
    let Some(r) = renderer() else { return };
    let models = BTreeMap::from([("rig", rig())]);
    let plain = render(r, scene(json!({}), model(json!({})), models.clone()), Quality::Q3);
    let rest = render(r, scene(json!({}), model(json!({ "shapes": { "bulge": 0 } })), models), Quality::Q3);
    assert_eq!(plain, rest);
}

#[test]
fn blend_shapes_of_the_file_widen_the_middle() {
    let Some(r) = renderer() else { return };
    let (thin, _) = posed(r, json!({ "shapes": { "bulge": 0 } }), &[0.0]);
    let (wide, _) = posed(r, json!({ "shapes": { "bulge": 1 } }), &[0.0]);
    assert!(!lit(&thin[0], 35, 21) && lit(&wide[0], 35, 21));
    assert!(lit(&wide[0], 32, 9) == lit(&thin[0], 32, 9), "the ends keep their width");
}

#[test]
fn root_motion_moves_the_layer_on_past_each_loop() {
    let Some(r) = renderer() else { return };
    let walk =
        json!({ "clips": [{ "id": "walk", "animation": "walk" }], "animations": [{ "clip": "walk" }] });
    let mut rooted = walk.clone();
    rooted["root_motion"] = json!(true);
    let (plain, _) = posed(r, walk, &[0.0, 1.5]);
    let (moved, shot) = posed(r, rooted, &[0.0, 1.5]);
    let travel = shot.runtime.puppet("rig").unwrap().posed().travel;
    assert!((travel[0] - 1.5).abs() < 0.05, "{travel:?}");
    assert!(lit(&plain[1], 37, 25) && !lit(&plain[1], 48, 25), "without root motion the loop starts over");
    assert!(lit(&moved[1], 48, 25), "with root motion the column walks on");
}

#[test]
fn a_clip_played_once_holds_its_pose_and_the_scene_sleeps() {
    let Some(r) = renderer() else { return };
    let play = json!({ "clips": [{ "id": "bend", "animation": "bend", "loop": "once" }], "animations": [{ "clip": "bend" }] });
    let (_, mut shot) = posed(r, play, &[0.0, 3.0, 0.5]);
    let frame = shot.runtime.frame(shot.view.view((SIDE, SIDE)), Quality::Q3);
    assert_eq!(frame.wake, Wake::Idle, "a clip played once holds its pose");
    let before = shot.view.model_renders();
    shot.frame(r, 0.5, Quality::Q3);
    assert_eq!(shot.view.model_renders(), before);
}
