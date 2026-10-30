use super::cloud::Event;
use super::spawn::KEY;
use super::{Particle, Sim};
use std::f32::consts::TAU;

fn up(a: f32, b: f32, t: f32) -> f32 {
    if b > a {
        let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    } else if t >= a {
        1.0
    } else {
        0.0
    }
}

fn window(w: [f32; 4], t: f32) -> f32 {
    up(w[0], w[1], t) * (1.0 - up(w[2], w[3], t))
}

fn reach(r: f32, f: [f32; 4]) -> f32 {
    let (radius, kind) = (f[0], f[1] as u32);
    if radius > 0.0 {
        let x = (1.0 - r / radius).max(0.0);
        return [1.0, x, x * x][kind.min(2) as usize];
    }
    [1.0, r / (r * r).max(1.0), 1.0 / (r * r).max(1.0)][kind.min(2) as usize]
}

fn wobble(p: &Particle, s: &Sim, k: usize, a: f32) -> [f32; 3] {
    let (f, g, w) = (s.forces[4 * k + 1], s.forces[4 * k + 2], s.forces[4 * k + 3]);
    let r = |j: u32| rand(s.ids[0], p.id, KEY + 64 + 3 * k as u32 + j);
    let (freq, amp, phase) = (lerp(f[0], f[1], r(0)), lerp(f[2], f[3], r(1)), lerp(g[0], g[1], r(2)));
    let amp = amp * window(w, (a / p.life).clamp(0.0, 1.0));
    let turn = TAU * (freq * a + phase);
    [amp * turn.sin(), amp * (turn + TAU / 4.0).sin(), amp]
}

pub(super) fn simulate(p: &mut Particle, s: &Sim, mirror: &[Event], points: &[[f32; 2]]) {
    let dt = s.clock[0];
    if s.mode[0] == 3 {
        follow(p, s, mirror);
    }
    let t = (p.age / p.life).clamp(0.0, 1.0);
    let (mut acc, mut drag, mut slow, mut damp) = ([0.0f32; 2], 0.0f32, 1.0f32, 0.0f32);
    let (mut shift, mut look) = ([0.0f32; 2], [1.0f32; 2]);
    for k in 0..(s.counts[2] as usize).min(16) {
        let (f, a, b) = (s.forces[4 * k], s.forces[4 * k + 1], s.forces[4 * k + 2]);
        if p.age >= p.life {
            break;
        }
        let w = window(s.forces[4 * k + 3], t);
        if w <= 0.0 {
            continue;
        }
        let d = [f[1] - p.pos[0], f[2] - p.pos[1]];
        let r2 = d[0] * d[0] + d[1] * d[1];
        let r = r2.sqrt().max(1e-3);
        let n = [d[0] / r, d[1] / r];
        let add = match f[0] as u32 {
            1 => [f[1], f[2]],
            2 => {
                drag += f[1] * w;
                [0.0; 2]
            }
            3 => {
                let at = [(p.pos[0] - a[0] * s.clock[1]) * f[2], p.pos[1] * f[2]];
                let c = curl(s.ids[0], at, s.clock[1] * a[1]);
                [c[0] * f[1] * a[2], c[1] * f[1] * a[3]]
            }
            4 => n.map(|v| v * f[3] * reach(r, a)),
            5 => {
                let mut m = f[3] * reach(r, a);
                if a[3] >= 0.0 {
                    m *= 1.0 - up(a[2], a[3], r);
                }
                let pull =
                    if b[3] > 0.0 { b[2] * ((r - b[0]) / b[1].max(1.0)).clamp(-1.0, 1.0) } else { 0.0 };
                [-n[1] * m + n[0] * pull, n[0] * m + n[1] * pull]
            }
            6 => {
                let (k, v2) = (f[3] * reach(r, a), p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]);
                [n[0] * v2 / r - n[1] * k - p.vel[0], n[1] * v2 / r + n[0] * k - p.vel[1]]
            }
            7 => {
                let j = s.ids[2].wrapping_mul(2).wrapping_add(16);
                [0, 1].map(|i| (rand(s.ids[0], p.id, j + i) * 2.0 - 1.0) * f[1])
            }
            8 => {
                let (now, next) = (wobble(p, s, k, p.age), wobble(p, s, k, p.age + dt));
                match f[1] as u32 {
                    0 => shift = [0, 1].map(|i| shift[i] + (next[i] - now[i]) * b[2 + i]),
                    1 => look[0] *= (1.0 - 0.5 * (next[2] + next[0])).clamp(0.0, 1.0),
                    _ => look[1] *= (1.0 + next[0]).max(0.0),
                }
                [0.0; 2]
            }
            10 => {
                p.spin += f[1] * w * dt;
                [0.0; 2]
            }
            11 => {
                damp += f[1] * w;
                [0.0; 2]
            }
            12 if f[3] > 0.0 => {
                slow *= lerp(a[2], 1.0, up(a[0], a[1], r)).max(0.0).powf(dt * 60.0 * w);
                [0.0; 2]
            }
            _ => [0.0; 2],
        };
        acc = [acc[0] + add[0] * w, acc[1] + add[1] * w];
    }
    let keep = (-drag * dt).exp() * slow;
    p.vel = [0, 1].map(|i| (p.vel[i] + acc[i] * dt) * keep);
    for k in 0..(s.counts[2] as usize).min(16) {
        let f = s.forces[4 * k];
        let w = window(s.forces[4 * k + 3], t);
        let l = p.vel[0].hypot(p.vel[1]);
        if f[0] as u32 == 9 && w > 0.0 && l > f[1] {
            let cut = lerp(1.0, f[1].max(0.0) / l, w);
            p.vel = p.vel.map(|v| v * cut);
        }
    }
    p.spin *= (-damp * dt).exp();
    p.pos = [0, 1].map(|i| p.pos[i] + p.vel[i] * dt + shift[i]);
    p.rot += p.spin * dt;
    p.age += dt;
    p.look = look;
    let (w, h) = (s.clock[2], s.clock[3]);
    for k in 0..(s.counts[3] as usize).min(8) {
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
            4 => {
                if let Some((d, g)) = super::field::sample(points, a, [w, h], p.pos) {
                    let l = g[0].hypot(g[1]);
                    hit(p, if l > 1e-6 { [g[0] / l, g[1] / l] } else { [0.0, -1.0] }, -d, b);
                }
            }
            _ => {}
        }
    }
}

fn follow(p: &mut Particle, s: &Sim, mirror: &[Event]) {
    let h = mirror.get(p.host[0] as usize).filter(|h| h.id == p.host[1] && h.alive != 0);
    let Some(h) = h else {
        p.age = p.life;
        return;
    };
    if s.hatch[1][1] < 0.5 {
        p.pos = [p.pos[0] + h.pos[0] - h.prev[0], p.pos[1] + h.pos[1] - h.prev[1]];
    }
    let bits = if s.hatch[0][3] > 0.5 { s.hatch[0][2] as u32 } else { 0 };
    if bits & 1 != 0 {
        p.color[..3].copy_from_slice(&h.color[..3]);
    }
    if bits & 2 != 0 {
        p.size = h.size * s.hatch[1][0];
    }
    if bits & 4 != 0 {
        p.color[3] = h.color[3];
    }
    if bits & 8 != 0 {
        p.vel = h.vel;
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

pub(super) fn bend(u: f32, x: f32) -> f32 {
    if x == 1.0 { u } else { u.powf(x) }
}

pub(super) fn pcg(v: u32) -> u32 {
    let s = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277_803_737);
    (w >> 22) ^ w
}

pub(crate) fn rand(seed: u32, id: u32, k: u32) -> f32 {
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

fn curl(seed: u32, p: [f32; 2], shift: f32) -> [f32; 2] {
    let (q, e) = ([p[0], p[1] + shift], 0.01);
    let n = |x: f32, y: f32| noise(seed, [q[0] + x, q[1] + y]);
    [(n(0.0, e) - n(0.0, -e)) / (2.0 * e), (n(-e, 0.0) - n(e, 0.0)) / (2.0 * e)]
}
