use super::*;

#[test]
fn sway_bends_the_top_and_swing_turns_around_the_pivot() {
    let input = Shot(ramp());
    let Some(out) = Fx::new("sway", json!({ "amplitude": 8 })).input(ramp()).shot() else { return };
    out.check(&[((10, 31), input.at(10, 31)), ((40, 31), input.at(40, 31))]);
    assert!((0..W).any(|x| out.at(x, 0)[0].abs_diff(input.at(x, 0)[0]) > 12));
    let swing = json!({ "swing": true, "angle": 20, "speed": 0.25 });
    let Some(mut run) = Fx::new("sway", swing).input(ramp()).run() else { return };
    run.runtime.advance(1.0);
    let out = run.shot();
    out.check(&[((32, 0), input.at(32, 0))]);
    assert!(out.at(32, 31)[0].abs_diff(input.at(32, 31)[0]) > 20, "{:?}", out.at(32, 31));
}
