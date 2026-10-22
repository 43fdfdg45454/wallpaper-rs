use super::{Particle, Sim};
use std::f32::consts::TAU;

pub(super) fn simulate(p: &mut Particle, s: &Sim) {
    let dt = s.clock[0];
    let (mut acc, mut drag) = ([0.0f32; 2], 0.0f32);
    for f in s.forces.iter().take(s.counts[2] as usize) {
        let d = [f[1] - p.pos[0], f[2] - p.pos[1]];
        let r2 = d[0] * d[0] + d[1] * d[1];
        let add = match f[0] as u32 {
            1 => [f[1], f[2]],
            2 => {
                drag += f[1];
                [0.0; 2]
            }
            3 => curl(s.ids[0], [p.pos[0] * f[2], p.pos[1] * f[2]], s.clock[1]).map(|c| c * f[1]),
            4 => d.map(|v| v * (f[3] / (r2.max(1.0) * r2.sqrt().max(1e-3)))),
            5 => [-d[1] * (f[3] / r2.max(1.0)), d[0] * (f[3] / r2.max(1.0))],
            6 => {
                let r = r2.sqrt().max(1e-3);
                let (n, v2) = ([d[0] / r, d[1] / r], p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]);
                [n[0] * v2 / r - n[1] * f[3] - p.vel[0], n[1] * v2 / r + n[0] * f[3] - p.vel[1]]
            }
            7 => {
                let k = s.ids[2].wrapping_mul(2).wrapping_add(16);
                [0, 1].map(|j| (rand(s.ids[0], p.id, k + j) * 2.0 - 1.0) * f[1])
            }
            _ => [0.0; 2],
        };
        acc = [acc[0] + add[0], acc[1] + add[1]];
    }
    let keep = (-drag * dt).exp();
    p.vel = [0, 1].map(|i| (p.vel[i] + acc[i] * dt) * keep);
    p.pos = [0, 1].map(|i| p.pos[i] + p.vel[i] * dt);
    p.rot += p.spin * dt;
    p.age += dt;
    let (w, h) = (s.clock[2], s.clock[3]);
    for k in 0..s.counts[3] as usize {
        let (a, b) = (s.colliders[2 * k], s.colliders[2 * k + 1]);
        match a[0] as u32 {
            1 => {
                hit(p, [1.0, 0.0], -p.pos[0], b);
                hit(p, [-1.0, 0.0], p.pos[0] - w, b);
                hit(p, [0.0, 1.0], -p.pos[1], b);
                hit(p, [0.0, -1.0], p.pos[1] - h, b);
            }
            2 => hit(p, [a[3], b[0]], -((p.pos[0] - a[1]) * a[3] + (p.pos[1] - a[2]) * b[0]), b),
            3 => {
                let d = [p.pos[0] - a[1], p.pos[1] - a[2]];
                let r = (d[0] * d[0] + d[1] * d[1]).sqrt();
                let n = if r > 1e-6 { [d[0] / r, d[1] / r] } else { [0.0, -1.0] };
                hit(p, n, a[3] - r, b);
            }
            _ => {}
        }
    }
}

fn hit(p: &mut Particle, n: [f32; 2], depth: f32, b: [f32; 4]) {
    if depth <= 0.0 {
        return;
    }
    if b[3] > 0.5 {
        p.age = p.life;
        return;
    }
    p.pos = [p.pos[0] + n[0] * depth, p.pos[1] + n[1] * depth];
    let vn = p.vel[0] * n[0] + p.vel[1] * n[1];
    if vn < 0.0 {
        p.vel = [0, 1].map(|i| (p.vel[i] - n[i] * vn) * (1.0 - b[2]) - n[i] * vn * b[1]);
    }
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn pcg(v: u32) -> u32 {
    let s = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277_803_737);
    (w >> 22) ^ w
}

pub(super) fn rand(seed: u32, id: u32, k: u32) -> f32 {
    (pcg(seed ^ pcg(id ^ pcg(k))) >> 8) as f32 / 16_777_216.0
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

fn curl(seed: u32, p: [f32; 2], t: f32) -> [f32; 2] {
    let (q, e) = ([p[0], p[1] + t * 0.2], 0.01);
    let n = |x: f32, y: f32| noise(seed, [q[0] + x, q[1] + y]);
    [(n(0.0, e) - n(0.0, -e)) / (2.0 * e), (n(-e, 0.0) - n(e, 0.0)) / (2.0 * e)]
}
