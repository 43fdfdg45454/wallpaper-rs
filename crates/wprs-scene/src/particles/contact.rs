use super::hatch::Event;
use super::{Particle, Sim};

pub(super) fn collide(p: &mut Particle, s: &Sim, points: &[[f32; 2]], prev: [f32; 2]) {
    let (w, h) = (s.clock[2], s.clock[3]);
    for k in 0..(s.counts[3] as usize).min(8) {
        let [a, b, c, r] = [0, 1, 2, 3].map(|i| s.colliders[4 * k + i]);
        match a[0] as u32 {
            1 => {
                hit(p, [1.0, 0.0], -p.pos[0], r);
                hit(p, [-1.0, 0.0], p.pos[0] - w, r);
                hit(p, [0.0, 1.0], -p.pos[1], r);
                hit(p, [0.0, -1.0], p.pos[1] - h, r);
            }
            2 => hit(p, [a[3], b[0]], -((p.pos[0] - a[1]) * a[3] + (p.pos[1] - a[2]) * b[0]), r),
            3 => {
                let d = [p.pos[0] - a[1], p.pos[1] - a[2]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                let n = if l > 1e-6 { [d[0] / l, d[1] / l] } else { [0.0, -1.0] };
                match b[0] > 0.5 {
                    true => hit(p, [-n[0], -n[1]], l - a[3], r),
                    false => hit(p, n, a[3] - l, r),
                }
            }
            4 => {
                if let Some((d, g)) = super::field::sample(points, a, [w, h], p.pos) {
                    let l = g[0].hypot(g[1]);
                    hit(p, if l > 1e-6 { [g[0] / l, g[1] / l] } else { [0.0, -1.0] }, -d, r);
                }
            }
            5 => super::walls::boxed(p, a, b, r),
            6 => super::walls::crossed(p, (a, b), prev, r),
            7 => super::walls::shaped(p, (a, b, c), points, r),
            _ => {}
        }
    }
}

pub(super) fn follow(p: &mut Particle, s: &Sim, mirror: &[Event]) {
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

pub(super) fn hit(p: &mut Particle, n: [f32; 2], depth: f32, r: [f32; 4]) {
    if depth <= 0.0 {
        return;
    }
    if r[2] > 0.5 {
        p.age = p.life;
        return;
    }
    p.pos = [p.pos[0] + n[0] * depth, p.pos[1] + n[1] * depth];
    if r[3] > 0.5 {
        (p.vel, p.spin, p.spare[0]) = ([0.0; 2], 0.0, 1.0);
        return;
    }
    let vn = p.vel[0] * n[0] + p.vel[1] * n[1];
    if vn < 0.0 {
        p.vel = [0, 1].map(|i| (p.vel[i] - n[i] * vn) * (1.0 - r[1]) - n[i] * vn * r[0]);
    }
}
