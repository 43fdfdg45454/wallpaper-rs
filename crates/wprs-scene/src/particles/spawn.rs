use super::hatch::Event;
use super::motion::{bend, lerp, rand};
use super::place::place;
use super::tone::tone;
use super::{Particle, Sim};

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
