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

#[test]
fn shine_styles_draw_two_lines_a_ring_or_a_glint() {
    let style = |n: u32| {
        let p = json!({ "angle": 0, "width": 0.25, "softness": 0, "intensity": 1, "speed": 0, "phase": 0.5,
            "style": n });
        Fx::new("shine", p).input(solid([64; 3])).shot()
    };
    let Some(line) = style(0) else { return };
    let Some(double) = style(1) else { return };
    let Some(ring) = style(2) else { return };
    let Some(glint) = style(3) else { return };
    assert!(double.at(32, 16)[0] > 150 && double.at(6, 16)[0] > 90, "{:?}", double.at(6, 16));
    assert!(line.at(6, 16)[0] < 70 && double.at(20, 16)[0] < 70, "{:?}", double.at(20, 16));
    assert!(ring.at(32, 16)[0] < 70 && ring.at(55, 16)[0] > 150, "{:?}", ring.at(55, 16));
    assert!(glint.at(32, 16)[0] > 150 && glint.at(44, 4)[0] < 70, "{:?}", glint.at(44, 4));
    assert!(glint.at(32, 24)[0] > 70 && glint.at(32, 24)[0] < glint.at(32, 16)[0], "{:?}", glint.at(32, 24));
    Shot::sheet(&[line, double, ring, glint]).looks_like("shine_shapes");
}
