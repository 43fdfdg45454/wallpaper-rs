use super::*;

fn speckles() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| {
        let v = (x * 37 + y * 91 + x * y * 13) % 256;
        Rgba([v as u8, (v * 3 % 256) as u8, (255 - v) as u8, 255])
    })
}

fn apart(a: &Shot, b: &Shot) -> (u32, f32) {
    let diffs = a.0.pixels().zip(b.0.pixels()).flat_map(|(p, q)| (0..3).map(move |c| p.0[c].abs_diff(q.0[c])));
    let (most, sum, n) = diffs.fold((0u8, 0u64, 0u64), |(m, s, n), d| (m.max(d), s + u64::from(d), n + 1));
    (u32::from(most), sum as f32 / n.max(1) as f32)
}

#[test]
fn a_magnified_layer_runs_its_effects_on_its_own_texels_without_blurring_them() {
    let white = json!({ "color": "#ffffff", "blend": "multiply" });
    let size = (W * 4, H * 4);
    let Some(mut tinted) = Fx::new("tint", white.clone()).input(speckles()).run() else { return };
    let Some(mut plain) = Fx::chain(json!([])).input(speckles()).run() else { return };
    let (a, b) = (tinted.shot_at(size), plain.shot_at(size));
    let (most, mean) = apart(&a, &b);
    assert!(most <= 3 && mean < 0.5, "most {most}, mean {mean}");
    let Some(mut lower) = Fx::new("tint", white).input(speckles()).quality(Quality::Q2).run() else { return };
    let (_, mean) = apart(&lower.shot_at(size), &b);
    assert!(mean < 24.0, "mean {mean}");
}
