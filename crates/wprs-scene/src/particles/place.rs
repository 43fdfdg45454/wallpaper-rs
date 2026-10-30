use super::Sim;
use super::motion::{lerp, rand};
use super::spawn::KEY;
use std::f32::consts::TAU;

pub(super) fn place(s: &Sim, e: usize, id: u32, points: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let (a, b, seed) = (s.emitters[4 * e], s.emitters[4 * e + 1], s.ids[0]);
    let u = [rand(seed, id, 0), rand(seed, id, 1)];
    let (w, h) = (s.clock[2], s.clock[3]);
    let middle = [w / 2.0, h / 2.0];
    match s.shapes[e] {
        1 if b[0] <= 0.0 && b[1] <= 0.0 => {
            ([lerp(a[0], a[2], u[0]), lerp(a[1], a[3], u[1])], [(a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0])
        }
        1 => (hollow(a, b, u, rand(seed, id, KEY + 8)), [(a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0]),
        2 => {
            let (r, t) = (lerp(a[3] * a[3], a[2] * a[2], u[0]).sqrt(), TAU * u[1]);
            let mut off = [r * t.cos() * b[0], r * t.sin() * b[1]];
            (0..2).filter(|k| b[2 + k] != 0.0).for_each(|k| off[k] = b[2 + k] * off[k].abs());
            ([a[0] + off[0], a[1] + off[1]], [a[0], a[1]])
        }
        3 => ([lerp(a[0], a[2], u[0]), lerp(a[1], a[3], u[0])], [(a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0]),
        4 => {
            let at = match u[0] * 2.0 * (w + h) {
                d if d < w => [d, 0.0],
                d if d < w + h => [w, d - w],
                d if d < 2.0 * w + h => [2.0 * w + h - d, h],
                d => [0.0, 2.0 * (w + h) - d],
            };
            (at, middle)
        }
        5 => {
            let i = a[0] as usize + ((u[0] * a[1]) as usize).min((a[1] as usize).saturating_sub(1));
            let p = points.get(i).copied().unwrap_or_default();
            ([p[0] + (u[1] - 0.5) * a[2], p[1] + (rand(seed, id, 11) - 0.5) * a[3]], middle)
        }
        _ => ([a[0], a[1]], [a[0], a[1]]),
    }
}

fn hollow(a: [f32; 4], b: [f32; 4], u: [f32; 2], pick: f32) -> [f32; 2] {
    let (lo, hi) = ([a[0].min(a[2]), a[1].min(a[3])], [a[0].max(a[2]), a[1].max(a[3])]);
    let size = [hi[0] - lo[0], hi[1] - lo[1]];
    let inner = [b[0].min(size[0]), b[1].min(size[1])];
    let band = [(size[0] - inner[0]) / 2.0, (size[1] - inner[1]) / 2.0];
    let (row, side) = (size[0] * band[1], band[0] * inner[1]);
    let at = pick * 2.0 * (row + side);
    let y = [lo[1] + band[1], hi[1] - band[1]];
    match at {
        at if at < row => [lerp(lo[0], hi[0], u[0]), lerp(lo[1], y[0], u[1])],
        at if at < 2.0 * row => [lerp(lo[0], hi[0], u[0]), lerp(y[1], hi[1], u[1])],
        at if at < 2.0 * row + side => [lerp(lo[0], lo[0] + band[0], u[0]), lerp(y[0], y[1], u[1])],
        _ => [lerp(hi[0] - band[0], hi[0], u[0]), lerp(y[0], y[1], u[1])],
    }
}
