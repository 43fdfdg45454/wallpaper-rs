use image::{Rgba, RgbaImage};
use wprs_core::edit::{Matrix, Stroke, mul, spline};

pub(crate) const LUT: usize = 1024;
const NEUTRAL: f64 = 6504.0;
const BRADFORD: Matrix = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
const BRADFORD_INV: Matrix = [
    [0.9869929, -0.1470543, 0.1599627],
    [0.4323053, 0.5183603, 0.0492912],
    [-0.0085287, 0.0400428, 0.9684867],
];
const RGB_XYZ: Matrix =
    [[0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041]];
const XYZ_RGB: Matrix = [
    [3.2404542, -1.5371385, -0.4985314],
    [-0.9692660, 1.8760108, 0.0415560],
    [0.0556434, -0.2040259, 1.0572252],
];

fn locus(t: f64) -> [f64; 2] {
    let u = (0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t * t)
        / (1.0 + 8.42420235e-4 * t + 7.08145163e-7 * t * t);
    let v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t)
        / (1.0 - 2.89741816e-5 * t + 1.61456053e-7 * t * t);
    [u, v]
}

fn white(t: f64, tint: f64) -> [f64; 3] {
    let ([u, v], [u2, v2]) = (locus(t), locus(t + 1.0));
    let len = (u2 - u).hypot(v2 - v).max(1e-12);
    let normal = [-(v2 - v) / len, (u2 - u) / len].map(|n| n * (u2 - u).signum() * tint * 3e-4);
    let (u, v) = (u + normal[0], v + normal[1]);
    let d = 2.0 * u - 8.0 * v + 4.0;
    let (x, y) = (3.0 * u / d, 2.0 * v / d);
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn apply(m: &Matrix, v: [f64; 3]) -> [f64; 3] {
    m.map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum())
}

pub(crate) fn balance(temperature: f32, tint: f32) -> Matrix {
    let source = apply(&BRADFORD, white(f64::from(temperature), f64::from(tint)));
    let target = apply(&BRADFORD, white(NEUTRAL, 0.0));
    let gain: Matrix =
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { target[i] / source[i] } else { 0.0 }));
    [&BRADFORD_INV, &gain, &BRADFORD, &RGB_XYZ].into_iter().fold(XYZ_RGB, |m, next| mul(&m, next))
}

pub(crate) fn curves(channels: [&[[f32; 2]]; 4]) -> Vec<u16> {
    let fs = channels.map(spline);
    (0..LUT)
        .flat_map(|i| {
            let x = i as f32 / (LUT - 1) as f32;
            fs.iter().map(move |f| half::f16::from_f32(f(x).clamp(0.0, 1.0)).to_bits())
        })
        .collect()
}

pub(crate) fn brush(masks: &[&[Stroke]], aspect: f32) -> RgbaImage {
    let segments = || {
        masks.iter().enumerate().flat_map(|(c, strokes)| {
            strokes.iter().flat_map(move |s| {
                let ends =
                    s.points.windows(2).map(|p| (p[0], p[1])).chain(s.points.first().map(|p| (*p, *p)));
                ends.map(move |(a, b)| (c, s, a, b))
            })
        })
    };
    let area: f32 = segments()
        .map(|(_, s, a, b)| {
            ((a[0] - b[0]).abs() * aspect + 2.0 * s.radius) * ((a[1] - b[1]).abs() + 2.0 * s.radius)
        })
        .sum();
    let height = (512.0 / aspect.max(1.0)).min((32e6 / area.max(1e-9)).sqrt()).max(8.0);
    let (w, h) = (((height * aspect).round() as u32).clamp(1, 2048), height.round() as u32);
    let mut out = RgbaImage::new(w, h);
    let (sx, sy) = (w as f32 / aspect, h as f32);
    for (channel, s, a, b) in segments() {
        let px = |p: [f32; 2]| [p[0] * aspect, p[1]];
        let (a, b, r) = (px(a), px(b), s.radius);
        let span = |i: usize, scale: f32, limit: u32| {
            let lo = ((a[i].min(b[i]) - r) * scale).floor().max(0.0) as u32;
            lo.min(limit)..((a[i].max(b[i]) + r) * scale).ceil().max(0.0).min(limit as f32) as u32
        };
        let ab = [b[0] - a[0], b[1] - a[1]];
        let len = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-9);
        for y in span(1, sy, h) {
            for x in span(0, sx, w) {
                let p = [(x as f32 + 0.5) / sx - a[0], (y as f32 + 0.5) / sy - a[1]];
                let t = ((p[0] * ab[0] + p[1] * ab[1]) / len).clamp(0.0, 1.0);
                let near = |e: f32, half: f32| (e.abs() - half).max(0.0);
                let d = near(p[0] - t * ab[0], 0.5 / sx).hypot(near(p[1] - t * ab[1], 0.5 / sy));
                let inner = r * s.hardness.min(0.999);
                let cover = 1.0 - ((d - inner) / (r - inner).max(1e-6)).clamp(0.0, 1.0);
                let Rgba(pixel) = out.get_pixel_mut(x, y);
                let value = (cover * 255.0).round() as u8;
                pixel[channel] =
                    if s.erase { pixel[channel].min(255 - value) } else { pixel[channel].max(value) };
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use wprs_core::edit::Stroke;

    #[test]
    fn brushes_reach_the_right_end_of_very_long_pictures() {
        for aspect in [1.0, 300.0, 5000.0] {
            let stroke = |x: f32| Stroke { radius: 0.4, hardness: 0.9, erase: false, points: vec![[x, 0.5]] };
            let (left, right) = (stroke(0.0), stroke(1.0));
            let mask = super::brush(&[std::slice::from_ref(&left), std::slice::from_ref(&right)], aspect);
            let (w, h) = mask.dimensions();
            assert!(w <= 2048 && h >= 1, "{aspect}: {w}x{h}");
            assert_eq!(mask.get_pixel(0, h / 2)[0], 255, "{aspect}");
            assert_eq!(mask.get_pixel(w - 1, h / 2)[1], 255, "{aspect}");
            assert_eq!(mask.get_pixel(w / 2, h / 2)[1], 0, "{aspect}");
        }
    }
}
