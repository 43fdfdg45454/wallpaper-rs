use super::motion::{lerp, rand};
use super::{Particle, Sim};
use std::f32::consts::TAU;

pub(super) fn spawn(s: &Sim, k: u32, points: &[[f32; 2]]) -> Particle {
    let e = (0..3).find(|e| k < s.births[*e]).unwrap_or(3);
    let id = s.ids[1].wrapping_add(k);
    let r = |i: u32| rand(s.ids[0], id, i);
    let (m, i, c) = (s.emitters[2 * e + 1], s.init, s.colors);
    let at = place(s, e, id, points);
    let angle = lerp(i[1][0], i[1][1], r(4)).to_radians();
    let speed = lerp(i[0][2], i[0][3], r(3));
    let tint = r(9);
    Particle {
        pos: [at[0] + m[0], at[1] + m[1]],
        vel: [angle.cos() * speed + m[2], angle.sin() * speed + m[3]],
        age: 0.0,
        life: lerp(i[0][0], i[0][1], r(2)),
        size: lerp(i[1][2], i[1][3], r(5)),
        rot: lerp(i[2][0], i[2][1], r(6)),
        spin: lerp(i[2][2], i[2][3], r(7)),
        frame: lerp(i[3][0], i[3][1], r(8)),
        rand: r(10),
        id,
        color: std::array::from_fn(|j| lerp(c[0][j], c[1][j], tint)),
    }
}

fn place(s: &Sim, e: usize, id: u32, points: &[[f32; 2]]) -> [f32; 2] {
    let (a, seed) = (s.emitters[2 * e], s.ids[0]);
    let u = [rand(seed, id, 0), rand(seed, id, 1)];
    let (w, h) = (s.clock[2], s.clock[3]);
    match s.shapes[e] {
        1 => [lerp(a[0], a[2], u[0]), lerp(a[1], a[3], u[1])],
        2 => {
            let (r, t) = (lerp(a[3] * a[3], a[2] * a[2], u[0]).sqrt(), TAU * u[1]);
            [a[0] + r * t.cos(), a[1] + r * t.sin()]
        }
        3 => [lerp(a[0], a[2], u[0]), lerp(a[1], a[3], u[0])],
        4 => match u[0] * 2.0 * (w + h) {
            d if d < w => [d, 0.0],
            d if d < w + h => [w, d - w],
            d if d < 2.0 * w + h => [2.0 * w + h - d, h],
            d => [0.0, 2.0 * (w + h) - d],
        },
        5 => {
            let i = a[0] as usize + ((u[0] * a[1]) as usize).min((a[1] as usize).saturating_sub(1));
            let p = points.get(i).copied().unwrap_or_default();
            [p[0] + (u[1] - 0.5) * a[2], p[1] + (rand(seed, id, 11) - 0.5) * a[3]]
        }
        _ => [a[0], a[1]],
    }
}
