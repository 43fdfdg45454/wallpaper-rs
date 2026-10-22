use super::*;

#[test]
fn twirl_turns_around_a_still_center() {
    let input = RgbaImage::from_fn(W, H, |x, y| match x.abs_diff(32) < 3 && y.abs_diff(16) < 3 {
        true => Rgba([255, 255, 0, 255]),
        false => *ramp().get_pixel(x, y),
    });
    let fx = Fx::new("twirl", json!({ "angle": 180, "radius": 12 })).input(input.clone());
    let Some(out) = fx.shot() else { return };
    let source = Shot(input);
    out.check(&[((32, 16), [255, 255, 0]), ((2, 2), source.at(2, 2)), ((60, 28), source.at(60, 28))]);
    assert!(out.at(32, 10)[0].abs_diff(source.at(32, 10)[0]) > 8, "{:?}", out.at(32, 10));
}
