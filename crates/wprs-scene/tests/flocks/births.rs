use super::*;

fn born(shape: Json, init: Json) -> Vec<Particle> {
    let mut system =
        still(json!({ "capacity": 2000, "emitters": [{ "shape": shape, "burst": { "count": 2000 } }] }));
    init.as_object().unwrap().iter().for_each(|(k, v)| system["init"][k] = v.clone());
    let mut fly = Fly::of(json!({ "a": system }));
    fly.frame(0.0);
    fly.alive(0)
}

fn mean(values: impl Iterator<Item = f32>) -> f32 {
    let all: Vec<f32> = values.collect();
    all.iter().sum::<f32>() / all.len().max(1) as f32
}

#[test]
fn hollow_boxes_and_partial_ellipses_place_births() {
    let hollow = born(json!({ "box": { "from": [0, 0], "to": [10, 10], "inner": [8, 8] } }), json!({}));
    assert!(hollow.iter().all(|p| !(1.01..8.99).contains(&p.pos[0]) || !(1.01..8.99).contains(&p.pos[1])));
    assert!(hollow.iter().any(|p| p.pos[0] < 1.0) && hollow.iter().any(|p| p.pos[1] > 9.0));
    let half = born(
        json!({ "circle": { "center": [50, 50], "radius": 10, "sign": [1, 0], "scale": [2, 0.5] } }),
        json!({}),
    );
    assert!(half.iter().all(|p| p.pos[0] >= 50.0 && p.pos[0] <= 70.01 && (p.pos[1] - 50.0).abs() <= 5.01));
    assert!(half.iter().any(|p| p.pos[0] > 65.0) && half.iter().any(|p| p.pos[1] < 46.0));
}

#[test]
fn initializers_lean_pick_colors_and_aim() {
    let point = json!({ "point": [100, 50] });
    let leaning = born(point.clone(), json!({ "size": [0, 10, 4], "alpha": [0.2, 0.4] }));
    let m = mean(leaning.iter().map(|p| p.size));
    assert!((m - 2.0).abs() < 0.25, "u^4 leans to the low end: {m}");
    assert!(leaning.iter().all(|p| (0.2..=0.4).contains(&p.color[3])));
    let green = born(point.clone(), json!({ "hsv": { "hue": [120, 120], "sat": [1, 1], "val": [1, 1] } }));
    assert!(
        green.iter().all(|p| p.color.iter().zip([0.0, 1.0, 0.0, 1.0]).all(|(a, b)| (a - b).abs() < 1e-3))
    );
    let stepped = born(point.clone(), json!({ "hsv": { "hue": [0, 240], "steps": 3 } }));
    let hues = |c: [f32; 4]| [c[0] > 0.5, c[1] > 0.5, c[2] > 0.5];
    assert!(stepped.iter().all(|p| {
        [[true, false, false], [false, true, false], [false, false, true]].contains(&hues(p.color))
    }));
    let listed = born(point.clone(), json!({ "palette": { "colors": ["#ff0000", "#0000ff"] } }));
    assert!(listed.iter().all(|p| p.color == [1.0, 0.0, 0.0, 1.0] || p.color == [0.0, 0.0, 1.0, 1.0]));
    assert!(listed.iter().any(|p| p.color[0] > 0.5) && listed.iter().any(|p| p.color[2] > 0.5));
    let moving = born(point, json!({ "velocity": [[10, -5], [10, -5]] }));
    assert!(moving.iter().all(|p| near(p.vel, [10.0, -5.0], 1e-3)));
    let ring = json!({ "circle": { "center": [100, 50], "radius": 20, "inner": 10 } });
    let outward = born(ring, json!({ "speed": [0, 0], "radial": [30, 30] }));
    let away = |p: &Particle| (p.pos[0] - 100.0) * p.vel[0] + (p.pos[1] - 50.0) * p.vel[1];
    assert!(outward.iter().all(|p| away(p) > 0.0 && (p.vel[0].hypot(p.vel[1]) - 30.0).abs() < 1e-2));
}

#[test]
fn births_inherit_the_motion_of_the_pointer() {
    let system = still(
        json!({ "init": { "life": [10, 10], "speed": [0, 0], "inherit": { "from": "cursor", "speed": [1, 1] } } }),
    );
    let mut fly = Fly::of(json!({ "a": system }));
    let pointer = PointerState {
        pos: Some([0.5, 0.5]),
        smooth: [0.5, 0.5],
        velocity: [0.1, 0.0],
        inside: true,
        ..PointerState::default()
    };
    fly.rt.set_pointer(pointer);
    fly.frame(0.0);
    assert!(fly.alive(0).iter().all(|p| near(p.vel, [20.0, 0.0], 1e-3)), "{:?}", fly.alive(0)[0].vel);
}
