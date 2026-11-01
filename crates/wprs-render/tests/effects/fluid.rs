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
    let Some(mut fire) = Fx::new("fluid", json!({ "source_strength": 40, "color": "#ffaa00" })).input(solid([20; 3])).run()
    else {
        return;
    };
    for _ in 0..20 {
        fire.runtime.advance(1.0 / 30.0);
        fire.shot();
    }
    let rising = fire.shot();
    assert!(rising.at(32, 26)[0] > 60 && fire.runtime.wake() == Wake::Frame, "{:?}", rising.at(32, 26));
    let Some(mut bent) = Fx::new("fluid", json!({ "source_strength": 40, "render": 3 })).input(squares()).run() else {
        return;
    };
    for _ in 0..20 {
        bent.runtime.advance(1.0 / 30.0);
        bent.shot();
    }
    let warped = bent.shot();
    assert!(warped.diff(&Shot(squares())) > 0.3 && warped.diff(&smoke) > 0.3);
}
