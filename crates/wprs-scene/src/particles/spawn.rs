use super::hatch::Event;
use super::motion::{bend, lerp, rand};
use super::tone::tone;
use super::{Particle, Sim};
use std::f32::consts::TAU;

pub(super) const KEY: u32 = 0x8000_0000;

pub(super) fn spawn(s: &Sim, e: usize, id: u32, points: &[[f32; 2]], ev: Option<&Event>) -> Particle {
    let r = |k: u32| rand(s.ids[0], id, k);
    let i = s.init;
    let pick = |lo: f32, hi: f32, k: u32, x: f32| lerp(lo, hi, bend(r(k), x));
    let (p, q) = (s.emitters[4 * e + 2], s.emitters[4 * e + 3]);
    let lin = |v: [f32; 2]| [p[0] * v[0] + p[2] * v[1], p[1] * v[0] + p[3] * v[1]];
    let (at, center) = place(s, e, id, points);
    let angle = pick(i[1][0], i[1][1], 4, i[6][2]).to_radians();
    let ahead = [angle.cos(), angle.sin()];
    let mut v = if i[9][2] > 0.5 {
        [pick(i[5][0], i[5][2], 14, i[8][1]), pick(i[5][1], i[5][3], 15, i[8][1])]
    } else {
        ahead.map(|a| a * pick(i[0][2], i[0][3], 3, i[6][1]))
    };
    let out = [at[0] - center[0], at[1] - center[1]];
    let l = out[0].hypot(out[1]);
    let dir = if l > 1e-4 { [out[0] / l, out[1] / l] } else { ahead };
    let radial = pick(i[4][0], i[4][1], 13, i[8][0]);
    v = [v[0] + dir[0] * radial, v[1] + dir[1] * radial];
    let carry = pick(i[4][2], i[4][3], KEY, 1.0);
    let v = lin(v);
    let mut vel = [v[0] + q[2] + i[9][0] * carry, v[1] + q[3] + i[9][1] * carry];
    let shifted = lin(at);
    let mut pos = [shifted[0] + q[0], shifted[1] + q[1]];
    let mut size = pick(i[1][2], i[1][3], 5, i[6][3]) * s.hatch[1][0];
    let mut color = tone(s, id);
    if i[3][2] >= 0.0 {
        color[3] = pick(i[3][2], i[3][3], 12, i[7][3]);
    }
    let mut host = [0; 2];
    if let Some(ev) = ev {
        let bits = s.hatch[0][2] as u32;
        pos = [pos[0] + ev.pos[0], pos[1] + ev.pos[1]];
        if bits & 1 != 0 {
            color[..3].copy_from_slice(&ev.color[..3]);
        }
        if bits & 2 != 0 {
            size = ev.size * s.hatch[1][0];
        }
        if bits & 4 != 0 {
            color[3] = ev.color[3];
        }
        if bits & 8 != 0 {
            vel = [vel[0] + ev.vel[0], vel[1] + ev.vel[1]];
        }
        host = [ev.slot, ev.id];
    }
    Particle {
        pos,
        vel,
        age: 0.0,
        life: pick(i[0][0], i[0][1], 2, i[6][0]),
        size,
        rot: pick(i[2][0], i[2][1], 6, i[7][0]),
        spin: pick(i[2][2], i[2][3], 7, i[7][1]),
        frame: pick(i[3][0], i[3][1], 8, i[7][2]),
        rand: r(10),
        id,
        color,
        host,
        look: [1.0; 2],
    }
}

fn place(s: &Sim, e: usize, id: u32, points: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
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
