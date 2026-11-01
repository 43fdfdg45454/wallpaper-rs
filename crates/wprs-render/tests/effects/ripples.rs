use super::*;
use wprs_scene::Wake;

fn rippled(params: Value, frames: usize) -> Option<(Shot, Shot, Run)> {
    let fx = Fx::new("water_ripple", params).input(squares());
    let fx = fx.image("blue", solid(BLUE), false).image("wall", solid([255; 3]), true);
    let mut run = fx.run()?;
    let rest = run.shot();
    run.pointer([0.5; 2]);
    run.runtime.set_pointer(PointerState { buttons: 1, ..*run.runtime.pointer() });
    for _ in 0..frames {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    Some((run.shot(), rest, run))
}

#[test]
fn ripples_tint_reflect_slow_down_lean_back_and_stop_at_walls() {
    let Some((plain, rest, _)) = rippled(json!({}), 6) else { return };
    let Some((walled, _, _)) = rippled(json!({ "walls": "wall" }), 6) else { return };
    assert!(plain.diff(&rest) > 1.0 && walled.diff(&rest) < 0.05, "{} {}", plain.diff(&rest), walled.diff(&rest));
    let Some((tinted, ..)) = rippled(json!({ "tint": "#ff0000" }), 6) else { return };
    assert!(tinted.mean()[0] > tinted.mean()[2] + 2.0, "{:?}", tinted.mean());
    let Some((mirrored, ..)) = rippled(json!({ "reflection": "blue" }), 6) else { return };
    assert!(mirrored.mean()[2] > plain.mean()[2] + 1.0, "{:?} {:?}", mirrored.mean(), plain.mean());
    let Some((slow, ..)) = rippled(json!({ "speed": 0.4 }), 6) else { return };
    let Some((far, ..)) = rippled(json!({ "perspective": 1 }), 6) else { return };
    differ(&[&plain, &slow, &far, &tinted, &mirrored], 0.2);
    let Some((_, rest, mut run)) = rippled(json!({ "speed": 2, "tint": "#ff8080" }), 6) else { return };
    run.runtime.set_pointer(PointerState { buttons: 0, ..*run.runtime.pointer() });
    while run.runtime.wake() == Wake::Frame && run.runtime.time() < 20.0 {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    assert!(run.runtime.time() < 8.0 && run.shot().diff(&rest) < 0.05, "{}", run.runtime.time());
    Shot::sheet(&[plain, slow, far, tinted, mirrored]).looks_like("ripple_styles");
}
