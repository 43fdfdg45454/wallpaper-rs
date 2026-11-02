use super::*;
use wprs_scene::Wake;

fn key(t: f32, x: f32) -> Value {
    json!({ "t": t, "eye": [x, 0, 5], "center": [x, 0, 0] })
}

fn flown(paths: Value, extra: Value) -> Scene {
    let models = BTreeMap::from([("red", cube(unlit("paint", [1.0, 0.0, 0.0, 1.0])))]);
    let mut camera = json!({ "paths": paths });
    extra.as_object().unwrap().iter().for_each(|(k, v)| camera[k] = v.clone());
    let layers = json!([{ "id": "cube", "type": "model", "model": "red", "scale": 0.5 }]);
    scene(json!({ "camera3d": camera }), layers, models)
}

fn red(px: &[u8], x: u32) -> u8 {
    rgb(px, x, 32)[0]
}

#[test]
fn a_path_flies_the_camera_and_holds_its_last_key_then_sleeps() {
    let Some(r) = renderer() else { return };
    let path = json!([{ "id": "pan", "keys": [key(0.0, 0.0), key(1.0, 1.5)] }]);
    let mut shot = shoot(r, flown(path, json!({})), &[]);
    let start = shot.frame(r, 0.0, Quality::Q3);
    let end = shot.frame(r, 1.5, Quality::Q3);
    assert!(red(&start, 32) > 200 && red(&end, 32) < 10 && red(&end, 16) > 200);
    assert_eq!(shot.runtime.frame(shot.view.view((SIDE, SIDE)), Quality::Q3).wake, Wake::Idle);
}

#[test]
fn a_looping_path_closes_without_a_jump() {
    let Some(r) = renderer() else { return };
    let path =
        json!([{ "id": "ring", "loop": true, "keys": [key(0.0, 0.0), key(1.0, 1.0), key(2.0, -1.0)] }]);
    let mut shot = shoot(r, flown(path, json!({})), &[]);
    let first = shot.frame(r, 0.0, Quality::Q3);
    let again = shot.frame(r, 3.0, Quality::Q3);
    assert_eq!(first, again, "one period later the picture is the same");
    let mut near = shoot(
        r,
        flown(
            json!([{ "id": "ring", "loop": true,
        "keys": [key(0.0, 0.0), key(1.0, 1.0), key(2.0, -1.0)] }]),
            json!({}),
        ),
        &[],
    );
    let before = near.frame(r, 2.98, Quality::Q3);
    let diff = (0..SIDE).filter(|x| red(&before, *x).abs_diff(red(&first, *x)) > 100).count();
    assert!(diff <= 2, "just before the end the camera is almost back: {diff}");
    assert_eq!(near.runtime.frame(near.view.view((SIDE, SIDE)), Quality::Q3).wake, Wake::Frame);
}

#[test]
fn paths_fade_through_black_or_cross_into_each_other() {
    let Some(r) = renderer() else { return };
    let two = json!([{ "id": "a", "keys": [key(0.0, 0.0), key(2.0, 0.0)] },
        { "id": "b", "keys": [key(0.0, 1.5), key(2.0, 1.5)] }]);
    let mut black = shoot(r, flown(two.clone(), json!({ "fade": { "to": "black", "duration": 1 } })), &[]);
    let mid = black.frame(r, 0.0, Quality::Q3);
    let dark = black.frame(r, 2.0, Quality::Q3);
    assert!(red(&mid, 32) < 10, "a path starts out of black");
    assert!(red(&dark, 32) < 10 && red(&dark, 16) < 10, "the switch is black");
    let lit = black.frame(r, 1.0, Quality::Q3);
    assert!(red(&lit, 16) > 200, "the next path shows once the fade is over");
    let mut cross = shoot(r, flown(two, json!({ "fade": { "to": "cross", "duration": 1 } })), &[]);
    let half = cross.frame(r, 1.5, Quality::Q3);
    assert!(
        (60..200).contains(&red(&half, 32)) && (60..200).contains(&red(&half, 16)),
        "{} {}",
        red(&half, 32),
        red(&half, 16)
    );
}
