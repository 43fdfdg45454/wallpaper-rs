use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Curve {
    pub points: Vec<[f64; 2]>,
    pub x: [f64; 2],
    pub y: [f64; 2],
}

pub fn plot(x: [f64; 2], n: usize, f: impl Fn(f64) -> f64) -> Curve {
    let n = n.max(2);
    let at = |i: usize| x[0] + (x[1] - x[0]) * i as f64 / (n - 1) as f64;
    let round = |v: f64| (v * 1e4).round() / 1e4;
    let points: Vec<[f64; 2]> =
        (0..n).map(|i| [round(at(i)), round(f(at(i)))]).filter(|p| p[1].is_finite()).collect();
    let lo = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let hi = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let pad = ((hi - lo) * 0.05).max(1e-3);
    let y = if lo <= hi { [lo - pad, hi + pad] } else { [0.0, 1.0] };
    Curve { points, x, y }
}
