use super::*;

#[test]
fn bloom_spreads_light_around_bright_pixels() {
    let spot = RgbaImage::from_fn(W, H, |x, y| {
        Rgba(if x.abs_diff(32) < 4 && y.abs_diff(16) < 4 { [255; 4] } else { [0, 0, 0, 255] })
    });
    let params = json!({ "threshold": 0.5, "intensity": 2, "radius": 8 });
    let Some(out) = Fx::new("bloom", params).input(spot).shot() else { return };
    assert!(out.at(40, 16)[0] > 20 && out.at(32, 16) == [255; 3], "{:?}", out.at(40, 16));
    out.check(&[((2, 2), [0, 0, 0])]);
}
