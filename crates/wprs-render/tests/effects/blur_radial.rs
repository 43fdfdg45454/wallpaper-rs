use super::*;

fn spot() -> RgbaImage {
    let lit = |x: u32, y: u32| (46..50).contains(&x) && (14..18).contains(&y);
    RgbaImage::from_fn(W, H, |x, y| Rgba(if lit(x, y) { [255; 4] } else { [0, 0, 0, 255] }))
}

#[test]
fn blur_radial_streaks_away_from_its_center() {
    let Some(out) = Fx::new("blur_radial", json!({ "strength": 0.4 })).input(spot()).shot() else { return };
    assert!(out.at(56, 16)[0] > 40, "outer streak {:?}", out.at(56, 16));
    out.check(&[((40, 16), [0; 3]), ((32, 16), [0; 3])]);
    let follow = Fx::new("blur_radial", json!({ "strength": 0.4, "follow": true })).input(spot());
    let Some(mut run) = follow.run() else { return };
    run.pointer([0.75, 0.5]);
    run.shot().check(&[((56, 16), [0; 3]), ((48, 16), [255; 3])]);
}

#[test]
fn blur_radial_eases_back_to_its_center_when_the_pointer_leaves() {
    let streak = |presence: f32| {
        let mut run = Fx::new("blur_radial", json!({ "strength": 0.4, "follow": true })).input(spot()).run()?;
        run.runtime.set_pointer(PointerState { smooth: [0.75, 0.5], presence, ..PointerState::default() });
        Some(run.shot().at(56, 16)[0])
    };
    let (Some(on), Some(half), Some(off)) = (streak(1.0), streak(0.5), streak(0.0)) else { return };
    assert!(on < half && half < off, "{on} {half} {off}");
}
