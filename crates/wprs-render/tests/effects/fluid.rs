use super::*;
use wprs_scene::{Quality, Wake};

fn stirred(params: Value, quality: Quality) -> Option<(Shot, Run)> {
    let mut run = Fx::new("fluid", params).input(solid([20; 3])).quality(quality).run()?;
    let rest = run.shot();
    for k in 0..12 {
        run.pointer([0.2 + k as f32 * 0.05, 0.5]);
        run.runtime.set_pointer(PointerState { velocity: [1.5, 0.0], ..*run.runtime.pointer() });
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    Some((rest, run))
}

#[test]
fn fluid_smoke_follows_the_pointer_then_settles_and_freezes_at_the_lowest_quality() {
    let Some((rest, mut run)) = stirred(json!({ "push": 4, "color": "#ff8000" }), Quality::Q3) else { return };
    let smoke = run.shot();
    assert!(smoke.mean()[0] > rest.mean()[0] + 1.0, "{:?}", smoke.mean());
    assert_eq!(run.runtime.wake(), Wake::Frame);
    run.runtime.set_pointer(PointerState { velocity: [0.0; 2], ..*run.runtime.pointer() });
    while run.runtime.wake() == Wake::Frame && run.runtime.time() < 30.0 {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    assert!(run.runtime.time() < 10.0, "{}", run.runtime.time());
    let settled = run.shot();
    run.runtime.advance(1.0);
    assert_eq!(run.shot().diff(&settled), 0.0);
    let Some((rest, mut low)) = stirred(json!({ "push": 4 }), Quality::Q0) else { return };
    assert!(low.shot().diff(&rest) < 0.05 && low.runtime.wake() != Wake::Frame);
    let Some(mut fire) = Fx::new("fluid", json!({ "emitters": [{ "from": [0.5, 0.9], "strength": 40 }], "color": "#ffaa00" })).input(solid([20; 3])).run()
    else {
        return;
    };
    for _ in 0..20 {
        fire.runtime.advance(1.0 / 30.0);
        fire.shot();
    }
    let rising = fire.shot();
    assert!(rising.at(32, 26)[0] > 60 && fire.runtime.wake() == Wake::Frame, "{:?}", rising.at(32, 26));
    let Some(mut bent) = Fx::new("fluid", json!({ "emitters": [{ "from": [0.5, 0.9], "strength": 40 }], "render": 3 })).input(squares()).run() else {
        return;
    };
    for _ in 0..20 {
        bent.runtime.advance(1.0 / 30.0);
        bent.shot();
    }
    let warped = bent.shot();
    assert!(warped.diff(&Shot(squares())) > 0.3 && warped.diff(&smoke) > 0.3);
}

fn emitted(emitters: Value) -> Option<Shot> {
    let mut run = Fx::new("fluid", json!({ "emitters": emitters, "color": "#ffaa00", "push": 0 })).input(solid([20; 3])).run()?;
    for _ in 0..20 {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    Some(run.shot())
}

#[test]
fn fluid_draws_from_every_point_and_line_emitter_it_lists() {
    let points = json!([{ "from": [0.15, 0.9], "strength": 40 }, { "from": [0.85, 0.9], "strength": 40 }]);
    let Some(two) = emitted(points) else { return };
    let (left, right, middle) = (two.at(10, 26)[0], two.at(54, 26)[0], two.at(32, 29)[0]);
    assert!(left > 60 && right > 60 && middle + 20 < left.min(right), "{left} {right} {middle}");
    let line = json!([{ "from": [0.05, 0.5], "to": [0.95, 0.5], "angle": 90, "strength": 30 }]);
    let Some(wide) = emitted(line) else { return };
    assert!(wide.at(6, 18)[0] > 60 && wide.at(58, 18)[0] > 60, "{:?} {:?}", wide.at(6, 18), wide.at(58, 18));
    let Some(none) = emitted(json!([])) else { return };
    assert!(none.at(32, 26)[0] < 30, "{:?}", none.at(32, 26));
}

#[test]
fn fluid_draws_keyframed_emitters_as_they_are_at_each_time() {
    let quiet = json!([{ "from": [0.85, 0.9], "strength": 0 }]);
    let keys = json!({ "keys": [[0, quiet], [0.1, [{ "from": [0.85, 0.9], "strength": 40 }]]] });
    let Some(keyed) = emitted(keys) else { return };
    assert!(keyed.at(54, 26)[0] > 60 && keyed.at(10, 26)[0] < 30, "{:?} {:?}", keyed.at(54, 26), keyed.at(10, 26));
    let Some(still) = emitted(quiet) else { return };
    assert!(still.at(54, 26)[0] < 30, "{:?}", still.at(54, 26));
}
