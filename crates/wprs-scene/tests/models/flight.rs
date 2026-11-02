use super::*;

fn path(keys: Value, closed: bool) -> CameraPath {
    serde_json::from_value(json!({ "id": "p", "keys": keys, "loop": closed })).unwrap()
}

fn base() -> Eye {
    Eye { at: [0.0; 3], center: [0.0; 3], up: [0.0, 1.0, 0.0], fov: 60.0, near: 0.1, far: 100.0, aspect: 1.0 }
}

fn ring() -> CameraPath {
    let keys = json!([{ "t": 0, "eye": [0, 0, 5], "center": 0 }, { "t": 1, "eye": [5, 0, 0], "center": 0 },
        { "t": 2, "eye": [0, 0, -5], "center": 0 }, { "t": 4, "eye": [-5, 0, 0], "center": 0, "fov": 30 }]);
    path(keys, true)
}

fn gap(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

#[test]
fn a_closed_path_loops_back_to_its_first_key_without_a_jump_in_place_or_heading() {
    let p = ring();
    let period = path_period(&p);
    assert!((period - 16.0 / 3.0).abs() < 1e-5, "one mean gap after the last key: {period}");
    let at = |t: f32| path_pose(&p, t, base()).at;
    assert!(gap(at(0.0), [0.0, 0.0, 5.0]) < 1e-4);
    assert!(gap(at(period), at(0.0)) < 1e-4);
    let h = 1e-3;
    let way = |a: [f32; 3], b: [f32; 3]| unit3(std::array::from_fn(|i| b[i] - a[i]));
    let (before, after) = (way(at(period - 2.0 * h), at(period - h)), way(at(0.0), at(h)));
    assert!(gap(at(period - h), at(h)) < 0.05, "the seam is continuous");
    assert!(gap(before, after) < 0.05, "and so is its heading: {before:?} {after:?}");
    assert!((path_pose(&p, 4.0, base()).fov - 30.0).abs() < 1e-3);
    assert!((path_pose(&p, 0.0, base()).fov - 60.0).abs() < 1e-3, "keys without fov keep the camera's");
}

#[test]
fn centripetal_curves_pass_through_their_keys_and_never_loop_on_close_ones() {
    let pts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.01, 0.0, 0.0], [2.0, 0.0, 0.0]];
    assert!(gap(centripetal(pts, 0.0), pts[1]) < 1e-5 && gap(centripetal(pts, 1.0), pts[2]) < 1e-5);
    for s in 1..10 {
        let x = centripetal(pts, s as f32 / 10.0)[0];
        assert!((1.0..=1.01 + 1e-4).contains(&x), "{x}");
    }
}

#[test]
fn random_order_plays_every_path_once_per_round_and_fades_between_them() {
    let mut order = [0usize; 5];
    for round in 0..8 {
        sequence(5, PathOrder::Random, round, &mut order);
        let mut seen = order;
        seen.sort();
        assert_eq!(seen, [0, 1, 2, 3, 4]);
    }
    let a = path(
        json!([{ "t": 0, "eye": [0, 0, 5], "center": 0 }, { "t": 2, "eye": [0, 0, 5], "center": 0 }]),
        false,
    );
    let b = path(
        json!([{ "t": 0, "eye": [3, 0, 5], "center": 0 }, { "t": 2, "eye": [3, 0, 5], "center": 0 }]),
        false,
    );
    let cam: Camera3d = serde_json::from_value(json!({ "fade": { "to": "black", "duration": 1 } })).unwrap();
    let paths = [a.clone(), b.clone()];
    let fade = |t: f32| play_paths(&paths, &cam, t, 60.0).unwrap().fade;
    assert!(fade(1.0) < 1e-4 && fade(2.0) > 0.99 && fade(1.75) > 0.4 && fade(2.25) > 0.4);
    let cam: Camera3d = serde_json::from_value(json!({ "fade": { "to": "cross", "duration": 1 } })).unwrap();
    let crossed = play_paths(&paths, &cam, 1.5, 60.0).unwrap();
    let (eye, k) = crossed.cross.unwrap();
    assert!((k - 0.5).abs() < 1e-4 && gap(eye.at, [3.0, 0.0, 5.0]) < 1e-4);
    let once = play_paths(&[a], &cam, 5.0, 60.0).unwrap();
    assert!(!once.moving, "a single open path holds its last key");
}

#[test]
fn path_keys_are_checked() {
    let bad = |paths: Value| {
        let doc = json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] }, "layers": [], "camera3d": { "paths": paths } });
        !Scene::from_json(&doc.to_string()).unwrap().validate(&BTreeMap::new()).is_empty()
    };
    let k = |t: f32| json!({ "t": t, "eye": [0, 0, 5], "center": 0 });
    assert!(!bad(json!([{ "id": "a", "keys": [k(0.0), k(1.0)] }])));
    assert!(bad(json!([{ "id": "a", "keys": [k(0.0)] }])), "one key is not a path");
    assert!(bad(json!([{ "id": "a", "keys": [k(1.0), k(0.0)] }])), "keys in order");
    assert!(bad(json!([{ "id": "a", "keys": [k(0.0), { "t": 1, "eye": 0, "center": 0, "fov": 200 }] }])));
    let many: Vec<Value> = (0..300).map(|i| k(i as f32)).collect();
    assert!(bad(json!([{ "id": "a", "keys": many }])), "at most 256 keys");
    let paths: Vec<Value> =
        (0..17).map(|i| json!({ "id": format!("p{i}"), "keys": [k(0.0), k(1.0)] })).collect();
    assert!(bad(json!(paths)), "at most 16 paths");
}
