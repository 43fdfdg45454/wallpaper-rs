use super::*;
use wprs_scene::{BLENDS, Blend};

fn level(v: u32, top: u32) -> u8 {
    (v * 255 / top) as u8
}

fn backdrop() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| Rgba([level(x, W - 1), level(y, H - 1), 128, 255]))
}

fn layer() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| Rgba([level(y, H - 1), 255 - level(x, W - 1), level(x, W - 1) / 2 + 64, 255]))
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn hsl(c: [f32; 3]) -> [f32; 3] {
    let (hi, lo) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let (d, l) = (hi - lo, (hi + lo) / 2.0);
    if d <= 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = if l < 0.5 { d / (hi + lo) } else { d / (2.0 - hi - lo) };
    let h = match () {
        _ if c[0] >= hi => (c[1] - c[2]) / d,
        _ if c[1] >= hi => 2.0 + (c[2] - c[0]) / d,
        _ => 4.0 + (c[0] - c[1]) / d,
    };
    [(h / 6.0).rem_euclid(1.0), s, l]
}

fn rgb([h, s, l]: [f32; 3]) -> [f32; 3] {
    if s <= 0.0 {
        return [l; 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    [1.0 / 3.0, 0.0, -1.0 / 3.0].map(|o: f32| match (h + o).rem_euclid(1.0) {
        t if 6.0 * t < 1.0 => p + (q - p) * 6.0 * t,
        t if 2.0 * t < 1.0 => q,
        t if 3.0 * t < 2.0 => p + (q - p) * (2.0 / 3.0 - t) * 6.0,
        _ => p,
    })
}

fn dodge(b: f32, s: f32) -> f32 {
    if s >= 1.0 { 1.0 } else { (b / (1.0 - s)).min(1.0) }
}

fn burn(b: f32, s: f32) -> f32 {
    if s <= 0.0 { 0.0 } else { (1.0 - (1.0 - b) / s).max(0.0) }
}

fn hard(b: f32, s: f32) -> f32 {
    if s < 0.5 { 2.0 * b * s } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - s) }
}

fn reflect(b: f32, s: f32) -> f32 {
    if s >= 1.0 { 1.0 } else { (b * b / (1.0 - s)).min(1.0) }
}

fn channel(mode: Blend, b: f32, s: f32) -> f32 {
    match mode {
        Blend::Normal => s,
        Blend::Add => b + s,
        Blend::Multiply => b * s,
        Blend::Screen => 1.0 - (1.0 - b) * (1.0 - s),
        Blend::Lighten => b.max(s),
        Blend::Darken => b.min(s),
        Blend::Overlay => hard(s, b),
        Blend::SoftLight if s < 0.5 => 2.0 * b * s + b * b * (1.0 - 2.0 * s),
        Blend::SoftLight => b.sqrt() * (2.0 * s - 1.0) + 2.0 * b * (1.0 - s),
        Blend::HardLight => hard(b, s),
        Blend::Difference => (b - s).abs(),
        Blend::Exclusion => b + s - 2.0 * b * s,
        Blend::ColorDodge => dodge(b, s),
        Blend::ColorBurn => burn(b, s),
        Blend::LinearBurn => (b + s - 1.0).max(0.0),
        Blend::LinearDodge => (b + s).min(1.0),
        Blend::VividLight if s < 0.5 => burn(b, 2.0 * s),
        Blend::VividLight => dodge(b, 2.0 * s - 1.0),
        Blend::LinearLight => (b + 2.0 * s - 1.0).clamp(0.0, 1.0),
        Blend::PinLight if s < 0.5 => b.min(2.0 * s),
        Blend::PinLight => b.max(2.0 * s - 1.0),
        Blend::HardMix => f32::from(u8::from(b + s >= 1.0)),
        Blend::Subtract => (b - s).max(0.0),
        Blend::Divide if s <= 0.0 => 1.0,
        Blend::Divide => (b / s).min(1.0),
        Blend::Reflect => reflect(b, s),
        Blend::Glow => reflect(s, b),
        Blend::Phoenix => b.min(s) - b.max(s) + 1.0,
        Blend::Average => (b + s) / 2.0,
        Blend::Negation => 1.0 - (1.0 - b - s).abs(),
        Blend::DiffuseLight => b + b * s,
        _ => unreachable!(),
    }
}

fn expected(mode: Blend, b: [u8; 3], s: [u8; 3]) -> [u8; 3] {
    let (b, s) = (b.map(|v| f32::from(v) / 255.0), s.map(|v| f32::from(v) / 255.0));
    let (hb, hs) = (hsl(b), hsl(s));
    let mixed = match mode {
        Blend::DarkerColor if lum(s) < lum(b) => s,
        Blend::LighterColor if lum(s) > lum(b) => s,
        Blend::DarkerColor | Blend::LighterColor => b,
        Blend::Hue => rgb([hs[0], hb[1], hb[2]]),
        Blend::Saturation => rgb([hb[0], hs[1], hb[2]]),
        Blend::Color => rgb([hs[0], hs[1], hb[2]]),
        Blend::Luminosity => rgb([hb[0], hb[1], hs[2]]),
        Blend::Tint => s.map(|v| v * b[0].max(b[1]).max(b[2])),
        _ => [0, 1, 2].map(|k| channel(mode, b[k], s[k])),
    };
    mixed.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[test]
fn every_blend_mode_mixes_layers_like_its_formula() {
    let mut shots = Vec::new();
    for mode in BLENDS {
        let name = serde_json::to_value(mode).unwrap();
        let top = json!({ "id": "top", "type": "image", "image": "top", "size": [W, H], "blend": name });
        let fx = Fx::chain(json!([])).input(backdrop()).image("top", layer(), false).layer(top).space("srgb");
        let Some(shot) = fx.shot() else { return };
        let (back, front) = (backdrop(), layer());
        let off = (0..W * H)
            .map(|i| (i % W, i / W))
            .filter(|&(x, y)| {
                let [b, s] = [&back, &front].map(|i| i.get_pixel(x, y).0).map(|p| [p[0], p[1], p[2]]);
                let want = expected(mode, b, s);
                shot.at(x, y).iter().zip(want).any(|(a, w)| a.abs_diff(w) > 4)
            })
            .count();
        assert!(off <= 24, "{name}: {off} pixels differ from the formula");
        shots.push(shot);
    }
    Shot::sheet(&shots).looks_like("blends");
}
