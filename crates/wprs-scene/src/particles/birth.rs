use super::fbm::{fbm, noise1};
use super::motion::{lerp, rand};
use super::spawn::KEY;
use super::{Particle, Sim};
use std::f32::consts::TAU;

pub fn swirl(forward: f32, scale: f32, x: f32) -> f32 {
    forward + (noise1(x) * 2.0 - 1.0) * scale * 180.0
}

pub(super) fn turbulent(s: &Sim, id: u32) -> Option<[f32; 2]> {
    let ([forward, scale, swept, offset], g) = (s.fields[0], s.fields[1]);
    if g[3] < 0.5 {
        return None;
    }
    let x = swept + g[2] + offset * rand(s.ids[0], id, KEY + 40) + (s.ids[0] & 1023) as f32;
    let a = swirl(forward, scale, x).to_radians();
    let v = lerp(g[0], g[1], rand(s.ids[0], id, KEY + 41));
    Some([a.cos() * v, a.sin() * v])
}

pub fn fractal(at: [f32; 2], [distance, scale, chaos, drift]: [f32; 4], g: [f32; 3], r: f32) -> [f32; 2] {
    let p = [at[0] * scale / 100.0 + r * chaos * 17.0 + drift, at[1] * scale / 100.0 + r * chaos * 31.0];
    let octaves = g[2] as i32;
    let norm = distance / (1.0 - 0.5f32.powi(octaves));
    let mut off = [fbm(p, octaves) * norm, fbm([p[0] + 5.2, p[1] + 1.3], octaves) * norm];
    (0..2).filter(|k| g[*k] != 0.0).for_each(|k| off[k] = g[k] * off[k].abs());
    [at[0] + off[0], at[1] + off[1]]
}

pub(super) fn shifted(s: &Sim, id: u32, at: [f32; 2]) -> [f32; 2] {
    let g = s.fields[3];
    if g[3] < 0.5 {
        return at;
    }
    fractal(at, s.fields[2], [g[0], g[1], g[2]], rand(s.ids[0], id, KEY + 42))
}

pub(super) fn depth(s: &Sim, id: u32) -> [f32; 2] {
    let pick = |d: [f32; 4], k: u32| {
        if d[3] > 0.5 { lerp(d[0], d[1], super::motion::bend(rand(s.ids[0], id, k), d[2])) } else { 0.0 }
    };
    [pick(s.depth[0], KEY + 44), pick(s.depth[1], KEY + 45)]
}

pub fn remapped(x: f32, row: [[f32; 4]; 3], k: u32) -> f32 {
    let ([_, i0, i1, _], [_, o0, o1, _], [wave, clamp_in, clamp_out, _]) = (row[0], row[1], row[2]);
    let t = (x - i0) / (i1 - i0);
    let t = if clamp_in > 0.5 { t.clamp(0.0, 1.0) } else { t };
    let t = match wave as u32 {
        1 => 0.5 + 0.5 * (TAU * t).sin(),
        2 => noise1(t + k as f32 * 13.7),
        _ => t,
    };
    let y = lerp(o0, o1, t);
    if clamp_out > 0.5 { y.clamp(o0.min(o1), o0.max(o1)) } else { y }
}

pub(super) fn remap(p: &mut Particle, s: &Sim) {
    for k in 0..(s.remap[0] as usize).min(8) {
        let row = [s.remaps[3 * k], s.remaps[3 * k + 1], s.remaps[3 * k + 2]];
        let speed = p.vel[0].hypot(p.vel[1]);
        let x = match row[0][0] as u32 {
            2 => p.life,
            3 => p.age,
            4 => speed,
            5 => rand(s.ids[0], p.id, KEY + 56 + k as u32),
            6 => (p.pos[0] - row[0][3]).hypot(p.pos[1] - row[2][3]),
            _ => row[0][3],
        };
        let y = remapped(x, row, k as u32);
        give(p, row[1][0] as u32, speed, |v| match row[1][3] as u32 {
            1 => v * y,
            2 => v + y,
            3 => v - y,
            _ => y,
        });
    }
}

pub(super) fn give(p: &mut Particle, out: u32, speed: f32, op: impl Fn(f32) -> f32) {
    match out {
        0 => p.size = op(p.size).max(0.0),
        1 => p.color[3] = op(p.color[3]).clamp(0.0, 1.0),
        2 => (0..3).for_each(|c| p.color[c] = op(p.color[c]).max(0.0)),
        3 => {
            let n = op(speed);
            p.vel = if speed > 1e-6 { p.vel.map(|v| v * n / speed) } else { [n, 0.0] };
        }
        4 => p.life = op(p.life).max(1e-3),
        5 => p.rot = op(p.rot),
        6 => p.spin = op(p.spin),
        _ => p.frame = op(p.frame).max(0.0),
    }
}
