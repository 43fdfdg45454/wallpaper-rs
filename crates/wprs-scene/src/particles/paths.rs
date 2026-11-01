use super::Sim;
use super::motion::{lerp, rand};
use super::spawn::KEY;
use std::f32::consts::PI;

pub(super) type Spot = ([f32; 2], [f32; 2], [f32; 2]);

pub fn turn(count: f32, mirror: bool, order: u32) -> (f32, u32) {
    let n = (count.max(1.0) as u32).max(1);
    if !mirror || n < 2 {
        return ((order % n) as f32, order / n);
    }
    let (period, j) = (2 * (n - 1), order % (2 * (n - 1)));
    ((if j < n { j } else { period - j }) as f32, order / period)
}

fn fract(x: f32) -> f32 {
    x - x.floor()
}

pub fn taper(f: f32, at: f32, size: f32) -> f32 {
    let d = if f < at { (at - f) / at.max(1e-3) } else { (f - at) / (1.0 - at).max(1e-3) };
    lerp(1.0, size, d.clamp(0.0, 1.0))
}

pub(super) fn sweep(s: &Sim, e: usize, id: u32, order: u32, points: &[[f32; 2]]) -> Option<Spot> {
    let (a, b) = (s.emitters[4 * e], s.emitters[4 * e + 1]);
    match s.shapes[e] {
        6 => {
            let (i, _) = turn(b[0], b[2] > 0.5, order);
            let steps = if a[3] >= 360.0 { b[0] } else { b[0] - 1.0 };
            let angle = (i / steps.max(1.0) * a[3] + b[1] + b[3]).to_radians();
            Some(([a[0] + a[2] * angle.cos(), a[1] + a[2] * angle.sin()], [a[0], a[1]], [1.0, 0.0]))
        }
        7 => {
            let (i, round) = turn(b[0], b[2] > 0.5, order);
            let f = if b[0] > 1.5 { i / (b[0] - 1.0) } else { 0.0 } * b[1];
            let (p, q) = (s.paths[2 * e], s.paths[2 * e + 1]);
            let d = [a[2] - a[0], a[3] - a[1]];
            let flip = if rand(s.ids[0], round, KEY + 48) < 0.5 { -1.0 } else { 1.0 };
            let bow = p[0] * (PI * f).sin() * if p[1] != 0.0 { p[1] } else { flip };
            let at = [a[0] + d[0] * f - d[1] * bow, a[1] + d[1] * f + d[0] * bow];
            let grow = if b[3] > 0.5 { taper(f, fract(p[2] + q[0]), p[3]) } else { 1.0 };
            Some((at, [(a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0], [grow, 0.0]))
        }
        8 => {
            let n = a[1] as usize;
            let k = ((rand(s.ids[0], id, 0) * a[1]) as usize).min(n.saturating_sub(1));
            let i = a[0] as usize + 2 * k;
            let (q, c) =
                (points.get(i).copied().unwrap_or_default(), points.get(i + 1).map_or(0.0, |c| c[0]));
            Some((q, [0.0; 2], [1.0, c]))
        }
        _ => None,
    }
}
