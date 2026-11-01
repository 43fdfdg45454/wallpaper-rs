use super::contact::follow;
use super::hatch::Event;
use super::wave::{curl, reach, up, window, wobble};
use super::{Particle, Sim};

pub(super) fn simulate(p: &mut Particle, s: &Sim, mirror: &[Event], points: &[[f32; 2]], nudge: [f32; 2]) {
    let (dt, prev) = (s.clock[0], p.pos);
    if p.spare[0] > 0.5 {
        p.age += dt;
        return;
    }
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
            13 => n.map(|v| v * (r - a[0]) * f[3]),
            14 => super::tether::between(f, a, p.pos),
            15 => {
                super::tether::evolve(p, s, k, w);
                [0.0; 2]
            }
            16 => nudge,
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
        let most = if f[0] as u32 == 16 { s.forces[4 * k + 1][1] } else { f[1] };
        let capped = f[0] as u32 == 9 || (f[0] as u32 == 16 && most > 0.0);
        if capped && w > 0.0 && l > most {
            let cut = lerp(1.0, most.max(0.0) / l, w);
            p.vel = p.vel.map(|v| v * cut);
        }
    }
    p.spin *= (-damp * dt).exp();
    p.pos = [0, 1].map(|i| p.pos[i] + p.vel[i] * dt + shift[i]);
    p.rot += p.spin * dt;
    p.age += dt;
    p.depth[0] += p.depth[1] * dt;
    p.look = look;
    super::contact::collide(p, s, points, prev);
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
