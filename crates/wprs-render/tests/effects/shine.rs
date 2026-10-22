use super::*;

#[test]
fn shine_sweeps_across_and_rests_when_still() {
    let still = json!({ "angle": 0, "width": 0.25, "softness": 0, "intensity": 1, "speed": 0, "phase": 0.5 });
    let Some(out) = Fx::new("shine", still).input(solid([64; 3])).shot() else { return };
    assert!(out.at(32, 16)[0] > 150, "band {:?}", out.at(32, 16));
    out.check(&[((4, 16), [64; 3]), ((60, 16), [64; 3])]);
    let moving = json!({ "angle": 0, "width": 0.25, "intensity": 1, "speed": 1, "gap": 0 });
    let Some(mut run) = Fx::new("shine", moving).input(solid([64; 3])).run() else { return };
    run.runtime.seek(0.5);
    let middle = run.shot();
    run.runtime.seek(0.8);
    let later = run.shot();
    assert!(middle.at(32, 16)[0] > later.at(32, 16)[0] + 40, "{:?}", later.at(32, 16));
    assert!(later.at(58, 16)[0] > middle.at(58, 16)[0] + 40, "{:?}", later.at(58, 16));
}
