use super::motion::{lerp, rand};
use super::spawn::KEY;
use super::{Particle, Sim};
use std::f32::consts::TAU;

pub(super) fn up(a: f32, b: f32, t: f32) -> f32 {
    if b > a {
        let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    } else if t >= a {
        1.0
    } else {
        0.0
    }
}

pub(super) fn window(w: [f32; 4], t: f32) -> f32 {
    up(w[0], w[1], t) * (1.0 - up(w[2], w[3], t))
}

pub(super) fn reach(r: f32, f: [f32; 4]) -> f32 {
    let (radius, kind) = (f[0], f[1] as u32);
    if radius > 0.0 {
        let x = (1.0 - r / radius).max(0.0);
        return [1.0, x, x * x][kind.min(2) as usize];
    }
    [1.0, r / (r * r).max(1.0), 1.0 / (r * r).max(1.0)][kind.min(2) as usize]
}

pub(super) fn wobble(p: &Particle, s: &Sim, k: usize, a: f32) -> [f32; 3] {
    let (f, g, w) = (s.forces[4 * k + 1], s.forces[4 * k + 2], s.forces[4 * k + 3]);
    let r = |j: u32| rand(s.ids[0], p.id, KEY + 64 + 3 * k as u32 + j);
    let (freq, amp, phase) = (lerp(f[0], f[1], r(0)), lerp(f[2], f[3], r(1)), lerp(g[0], g[1], r(2)));
    let amp = amp * window(w, (a / p.life).clamp(0.0, 1.0));
    let turn = TAU * (freq * a + phase);
    [amp * turn.sin(), amp * (turn + TAU / 4.0).sin(), amp]
}

fn noise(seed: u32, p: [f32; 2]) -> f32 {
    let (i, j) = (p[0].floor(), p[1].floor());
    let f = [p[0] - i, p[1] - j];
    let u = f.map(|v| v * v * (3.0 - 2.0 * v));
    let g = |x: i32, y: i32| {
        let a = rand(seed, (i as i32).wrapping_add(x) as u32, (j as i32).wrapping_add(y) as u32) * TAU;
        a.cos() * (f[0] - x as f32) + a.sin() * (f[1] - y as f32)
    };
    lerp(lerp(g(0, 0), g(1, 0), u[0]), lerp(g(0, 1), g(1, 1), u[0]), u[1])
}

pub(super) fn curl(seed: u32, p: [f32; 2], shift: f32) -> [f32; 2] {
    let (q, e) = ([p[0], p[1] + shift], 0.01);
    let n = |x: f32, y: f32| noise(seed, [q[0] + x, q[1] + y]);
    [(n(0.0, e) - n(0.0, -e)) / (2.0 * e), (n(-e, 0.0) - n(e, 0.0)) / (2.0 * e)]
}
