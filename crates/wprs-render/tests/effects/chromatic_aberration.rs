use super::*;

#[test]
fn chromatic_aberration_splits_channels_away_from_the_center() {
    let edge = RgbaImage::from_fn(W, H, |x, _| Rgba(if x >= 48 { [255; 4] } else { [0, 0, 0, 255] }));
    let Some(out) = Fx::new("chromatic_aberration", json!({ "amount": 8 })).input(edge).shot() else {
        return;
    };
    out.check(&[
        ((47, 16), [255, 0, 0]),
        ((49, 16), [255, 255, 0]),
        ((32, 16), [0; 3]),
        ((60, 16), [255; 3]),
    ]);
}
