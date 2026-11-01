use super::birth::{give, remapped};
use super::fit::live;
use super::motion::{lerp, rand};
use super::points::Room;
use super::sow::table;
use super::spawn::KEY;
use super::{Particle, Remap, Sim};
use crate::resolve::Context;

pub(super) fn steady(r: &Remap, room: &Room, cx: &Context) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let row = table(r);
    let (x, y) = live(cx, room, &r.input).unwrap_or((0.0, 0.0));
    let clamps = row[2][1] + 2.0 * row[2][2];
    (
        [15.0, row[0][0], x, y],
        [row[0][1], row[0][2], row[1][1], row[1][2]],
        [row[1][0], row[1][3], row[2][0], clamps],
    )
}

pub(super) fn between(f: [f32; 4], a: [f32; 4], pos: [f32; 2]) -> [f32; 2] {
    let d = [a[0] - f[1], a[1] - f[2]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let along = ((pos[0] - f[1]) * d[0] + (pos[1] - f[2]) * d[1]) / l2;
    let t = if l2 > 1e-6 { along.clamp(0.0, 1.0) } else { 0.0 };
    [(f[1] + d[0] * t - pos[0]) * f[3], (f[2] + d[1] * t - pos[1]) * f[3]]
}

pub(super) fn evolve(p: &mut Particle, s: &Sim, k: usize, w: f32) {
    let dt = s.clock[0];
    let (f, a, b) = (s.forces[4 * k], s.forces[4 * k + 1], s.forces[4 * k + 2]);
    let bits = b[3] as u32;
    let row = [
        [f[1], a[0], a[1], f[2]],
        [b[0], a[2], a[3], b[1]],
        [b[2], (bits & 1) as f32, (bits >> 1 & 1) as f32, f[3]],
    ];
    let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
    let x = match f[1] as u32 {
        1 => s.clock[1],
        2 => p.life,
        3 => p.age,
        4 => speed,
        5 => rand(s.ids[0], p.id, KEY + 64 + k as u32),
        6 => {
            let d = [p.pos[0] - f[2], p.pos[1] - f[3]];
            (d[0] * d[0] + d[1] * d[1]).sqrt()
        }
        _ => f[2],
    };
    let y = remapped(x, row, k as u32 + 8);
    give(p, b[0] as u32, speed, |v| match b[1] as u32 {
        1 => v * y.max(0.0).powf(dt * w),
        2 => v + y * dt * w,
        3 => v - y * dt * w,
        _ => lerp(v, y, w),
    });
}
