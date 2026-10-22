use image::{Rgba, RgbaImage};
use wprs_core::edit::{Matrix, Stroke, mul};

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

fn spline(points: &[[f32; 2]]) -> impl Fn(f32) -> f32 + '_ {
    let n = points.len();
    let d: Vec<f32> =
        points.windows(2).map(|p| (p[1][1] - p[0][1]) / (p[1][0] - p[0][0]).max(1e-6)).collect();
    let mut m: Vec<f32> = (0..n)
        .map(|i| match i {
            0 => d.first().copied().unwrap_or(1.0),
            i if i == n - 1 => d[i - 1],
            i if d[i - 1] * d[i] <= 0.0 => 0.0,
            i => (d[i - 1] + d[i]) / 2.0,
        })
        .collect();
    for (i, slope) in d.iter().enumerate() {
        let (a, b) = (m[i] / slope, m[i + 1] / slope);
        if *slope == 0.0 {
            (m[i], m[i + 1]) = (0.0, 0.0);
        } else if a * a + b * b > 9.0 {
            let t = 3.0 / (a * a + b * b).sqrt();
            (m[i], m[i + 1]) = (t * a * slope, t * b * slope);
        }
    }
    move |x| {
        if n < 2 {
            return x;
        }
        let x = x.clamp(points[0][0], points[n - 1][0]);
        let i = points.windows(2).position(|p| x <= p[1][0]).unwrap_or(n - 2);
        let ([x0, y0], [x1, y1]) = (points[i], points[i + 1]);
        let (h, t) = (x1 - x0, (x - x0) / (x1 - x0).max(1e-6));
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * y0
            + (t3 - 2.0 * t2 + t) * h * m[i]
            + (-2.0 * t3 + 3.0 * t2) * y1
            + (t3 - t2) * h * m[i + 1]
    }
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
    for (channel, s, a, b) in segments() {
        let px = |p: [f32; 2]| [p[0] * aspect * h as f32, p[1] * h as f32];
        let (a, b, r) = (px(a), px(b), s.radius * h as f32);
        let span = |i: usize, limit: u32| {
            let lo = (a[i].min(b[i]) - r).floor().max(0.0) as u32;
            lo.min(limit)..((a[i].max(b[i]) + r).ceil().max(0.0) as u32).min(limit)
        };
        let ab = [b[0] - a[0], b[1] - a[1]];
        let len = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-9);
        for y in span(1, h) {
            for x in span(0, w) {
                let p = [x as f32 + 0.5 - a[0], y as f32 + 0.5 - a[1]];
                let t = ((p[0] * ab[0] + p[1] * ab[1]) / len).clamp(0.0, 1.0);
                let d = (p[0] - t * ab[0]).hypot(p[1] - t * ab[1]);
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
