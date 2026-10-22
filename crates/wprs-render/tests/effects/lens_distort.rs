use super::*;

#[test]
fn lens_distort_bulges_the_middle_and_keeps_the_corners() {
    let Some(out) = Fx::new("lens_distort", json!({ "k1": 0.3 })).input(ramp()).shot() else { return };
    let input = Shot(ramp());
    out.check(&[((32, 16), input.at(32, 16)), ((0, 0), input.at(0, 0)), ((63, 31), input.at(63, 31))]);
    assert!(out.at(16, 16)[0] > input.at(16, 16)[0] + 6, "{:?}", out.at(16, 16));
    let Some(flat) = Fx::new("lens_distort", json!({ "k1": 0 })).input(ramp()).shot() else { return };
    assert!(flat.diff(&input) < 0.5);
}
