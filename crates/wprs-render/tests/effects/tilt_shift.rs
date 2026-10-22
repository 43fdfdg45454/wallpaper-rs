use super::*;

fn stripes() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, _| Rgba(if x % 4 < 2 { [255; 4] } else { [0, 0, 0, 255] }))
}

fn contrast(shot: &Shot, xs: std::ops::Range<u32>, y: u32) -> u8 {
    let values = xs.map(|x| shot.at(x, y)[0]);
    values.clone().max().unwrap_or(0) - values.min().unwrap_or(0)
}

#[test]
fn tilt_shift_keeps_its_band_or_depth_sharp() {
    let band = Fx::new("tilt_shift", json!({ "width": 0.1, "radius": 8 })).input(stripes());
    let Some(out) = band.shot() else { return };
    let (sharp, soft) = (contrast(&out, 8..56, 16), contrast(&out, 8..56, 1));
    assert!(sharp > 240 && soft < 60, "band {sharp}, edge {soft}");
    let depth = Fx::new("tilt_shift", json!({ "depth": "depth", "focus": 0, "width": 0.1, "radius": 8 }))
        .input(stripes())
        .image("depth", halves([0; 3], [255; 3]), true);
    let Some(out) = depth.shot() else { return };
    let (near, far) = (contrast(&out, 4..24, 8), contrast(&out, 40..60, 8));
    assert!(near > 240 && far < 60, "near {near}, far {far}");
}
