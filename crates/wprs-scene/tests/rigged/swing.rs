use super::kit::{at, close, layers, runtime};
use serde_json::json;
use wprs_scene::{Quality, Runtime, Wake};

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

fn hanging(more: serde_json::Value) -> Runtime {
    let mut body = json!({ "bone": "root", "mode": "rope", "gravity": 2, "stiffness": 0, "damping": 0.5 });
    more.as_object().unwrap().iter().for_each(|(k, v)| body[k] = v.clone());
    runtime(json!({ "physics": [body] }))
}

fn run(rt: &mut Runtime, seconds: f32, dt: f32) {
    (0..(seconds / dt).round() as usize).for_each(|_| rt.advance(f64::from(dt)));
}

#[test]
fn a_rope_falls_and_settles_hanging_from_its_root() {
    let mut rt = hanging(json!({}));
    run(&mut rt, 0.25, 1.0 / 60.0);
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame, "a falling rope asks for frames");
    run(&mut rt, 20.0, 1.0 / 60.0);
    assert!(close(at(&rt, "arm"), [-24.0, -24.0], 0.05), "{:?}", at(&rt, "arm"));
    assert!(close(at(&rt, "hand"), [-24.0, -48.0], 0.1), "{:?}", at(&rt, "hand"));
    assert!(rt.puppet("fox").unwrap().settled());
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle, "a settled rope asks for no frames");
    let generation = rt.puppet("fox").unwrap().posed().generation;
    run(&mut rt, 2.0, 1.0 / 60.0);
    assert_eq!(rt.puppet("fox").unwrap().posed().generation, generation);
}

#[test]
fn the_fixed_step_does_not_depend_on_the_frame_rate() {
    let mut sixty = hanging(json!({}));
    let mut thirty = hanging(json!({}));
    let mut fast = hanging(json!({}));
    run(&mut sixty, 0.5, 1.0 / 60.0);
    run(&mut thirty, 0.5, 1.0 / 30.0);
    run(&mut fast, 0.5, 1.0 / 240.0);
    assert!(
        close(at(&sixty, "hand"), at(&thirty, "hand"), 1e-3),
        "{:?} {:?}",
        at(&sixty, "hand"),
        at(&thirty, "hand")
    );
    assert!(
        close(at(&sixty, "hand"), at(&fast, "hand"), 1.5),
        "{:?} {:?}",
        at(&sixty, "hand"),
        at(&fast, "hand")
    );
    let mut gap = hanging(json!({}));
    let mut steps = hanging(json!({}));
    gap.advance(10.0);
    run(&mut steps, 0.25, 1.0 / 60.0);
    assert!(close(at(&gap, "hand"), at(&steps, "hand"), 1e-3), "a long gap runs at most 15 steps");
}

#[test]
fn a_spring_lags_behind_its_clip_and_comes_back_to_it() {
    let mut rt = runtime(json!({ "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }],
        "animations": [{ "clip": "wave" }], "physics": [{ "bone": "arm", "stiffness": 0.2, "inertia": 1 }] }));
    let mut plain = runtime(json!({ "clips": [{ "id": "wave", "animation": "wave", "loop": "once" }],
        "animations": [{ "clip": "wave" }] }));
    run(&mut rt, 0.5, 1.0 / 60.0);
    run(&mut plain, 0.5, 1.0 / 60.0);
    assert!(!close(at(&rt, "hand"), at(&plain, "hand"), 0.5), "the hand lags behind");
    run(&mut rt, 10.0, 1.0 / 60.0);
    run(&mut plain, 10.0, 1.0 / 60.0);
    assert!(
        close(at(&rt, "hand"), at(&plain, "hand"), 0.05),
        "{:?} {:?}",
        at(&rt, "hand"),
        at(&plain, "hand")
    );
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
}

#[test]
fn limits_bound_the_swing_and_the_stretch() {
    let mut rt = hanging(json!({ "rotation": [-10, 10] }));
    run(&mut rt, 20.0, 1.0 / 60.0);
    let a = 10f32.to_radians();
    assert!(close(at(&rt, "arm"), [-24.0 + 24.0 * a.cos(), -24.0 * a.sin()], 0.05), "{:?}", at(&rt, "arm"));
    let mut rt = runtime(
        json!({ "physics": [{ "bone": "root", "mode": "spring", "gravity": 10, "stiffness": 0.2, "translation": [-5, 5] }] }),
    );
    run(&mut rt, 20.0, 1.0 / 60.0);
    assert!(close(at(&rt, "root"), [-24.0, -5.0], 0.01), "{:?}", at(&rt, "root"));
}

#[test]
fn the_lowest_quality_freezes_bones_and_the_battery_halves_the_rate() {
    let mut rt = hanging(json!({}));
    rt.set_quality(Quality::Q0);
    run(&mut rt, 2.0, 1.0 / 60.0);
    assert!(close(at(&rt, "hand"), [24.0, 0.0], 1e-4), "{:?}", at(&rt, "hand"));
    assert_eq!(rt.frame(VIEW, Quality::Q0).wake, Wake::Idle);
    let mut rt = hanging(json!({}));
    rt.set_battery(true);
    run(&mut rt, 20.0, 1.0 / 60.0);
    assert!(close(at(&rt, "hand"), [-24.0, -48.0], 0.1), "{:?}", at(&rt, "hand"));
    assert!(at(&rt, "hand").iter().all(|v| v.is_finite()));
}

#[test]
fn wind_keeps_ropes_moving_and_pushes_them_along() {
    let rope = json!([{ "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
        "physics": [{ "bone": "root", "mode": "rope", "gravity": 1, "stiffness": 0, "wind": 1 }] }]);
    let mut calm = layers(rope.clone());
    let mut scene = (*layers(rope).scene).clone();
    scene.canvas.wind = Some(wprs_scene::Value::Fixed(wprs_scene::Vec2([200.0, 0.0])));
    let mut windy = Runtime::new(std::sync::Arc::new(scene), Default::default());
    run(&mut windy, 10.0, 1.0 / 60.0);
    run(&mut calm, 10.0, 1.0 / 60.0);
    assert!(
        at(&windy, "hand")[0] > at(&calm, "hand")[0] + 5.0,
        "{:?} {:?}",
        at(&windy, "hand"),
        at(&calm, "hand")
    );
    assert_eq!(windy.frame(VIEW, Quality::Q3).wake, Wake::Frame, "gusts keep moving");
    run(&mut calm, 20.0, 1.0 / 60.0);
    assert_eq!(calm.frame(VIEW, Quality::Q3).wake, Wake::Idle);
}

#[test]
fn an_impulse_from_a_script_swings_a_settled_bone() {
    let mut rt = hanging(json!({}));
    run(&mut rt, 20.0, 1.0 / 60.0);
    let rest = at(&rt, "hand");
    let push = json!({ "rigs": { "fox": { "impulses": [["arm", [600, 0]]] } } });
    rt.set_script(serde_json::from_value(push).unwrap());
    run(&mut rt, 0.2, 1.0 / 60.0);
    assert!(!close(at(&rt, "hand"), rest, 1.0), "{:?} {:?}", at(&rt, "hand"), rest);
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
}
