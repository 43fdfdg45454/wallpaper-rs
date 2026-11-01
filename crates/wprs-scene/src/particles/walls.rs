use super::contact::hit;
use super::drive::NONE;
use super::points::bare;
use super::{MAX_COLLIDERS2, Particle, Sim, Surface, System};
use crate::Affine;

fn flag(b: bool) -> f32 {
    f32::from(u8::from(b))
}

pub(super) fn colliders(sim: &mut Sim, s: &System, frame: Affine) {
    let grow = (frame.0[0] * frame.0[3] - frame.0[1] * frame.0[2]).abs().sqrt();
    let lin = bare(frame);
    for (k, c) in s.collide.iter().take(MAX_COLLIDERS2).enumerate() {
        let (head, a) = match &c.bounds {
            Surface::Canvas => ([1.0, 0.0, 0.0, 0.0], NONE),
            Surface::Plane { point, normal } => {
                let ([x, y], [u, v]) = (frame.apply(*point), lin.apply(*normal));
                let l = u.hypot(v).max(1e-6);
                ([2.0, x, y, u / l], [v / l, 0.0, 0.0, 0.0])
            }
            Surface::Circle { center, radius, inside } => {
                let [x, y] = frame.apply(*center);
                ([3.0, x, y, radius * grow], [flag(*inside), 0.0, 0.0, 0.0])
            }
            Surface::Mask { .. } => ([4.0, 0.0, 0.0, 0.0], NONE),
            Surface::Box { from, to, inside } => {
                let mid = frame.apply([(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0]);
                let ([ux, uy], [vx, vy]) = (lin.apply([1.0, 0.0]), lin.apply([0.0, 1.0]));
                let (lu, lv) = (ux.hypot(uy).max(1e-6), vx.hypot(vy).max(1e-6));
                let half = [(to[0] - from[0]).abs() / 2.0 * lu, (to[1] - from[1]).abs() / 2.0 * lv];
                ([5.0, mid[0], mid[1], flag(*inside)], [ux / lu, uy / lu, half[0], half[1]])
            }
            Surface::Segment { from, to } => {
                let ([ax, ay], [bx, by]) = (frame.apply(*from), frame.apply(*to));
                ([6.0, ax, ay, 0.0], [bx, by, 0.0, 0.0])
            }
            Surface::Layer { .. } => ([0.0; 4], NONE),
        };
        let response = [c.bounce, c.friction, flag(c.kill), flag(c.stick)];
        sim.colliders[4 * k..4 * k + 4].copy_from_slice(&[head, a, NONE, response]);
    }
}

pub(super) fn layered(sim: &mut Sim, s: &System, draws: &crate::Frame, to_sim: Affine) {
    for (k, c) in s.collide.iter().take(MAX_COLLIDERS2).enumerate() {
        let Surface::Layer { id } = &c.bounds else { continue };
        let Some(d) = draws.draws.iter().find(|d| d.layer.id == *id) else { continue };
        let m = to_sim.then(d.world).0;
        sim.colliders[4 * k] = [7.0, 0.0, 0.0, 0.0];
        sim.colliders[4 * k + 1] = [m[0], m[1], m[2], m[3]];
        sim.colliders[4 * k + 2] = [m[4], m[5], 0.0, 0.0];
    }
}

pub(super) fn boxed(p: &mut Particle, a: [f32; 4], b: [f32; 4], r: [f32; 4]) {
    let (u, v) = ([b[0], b[1]], [-b[1], b[0]]);
    let d = [p.pos[0] - a[1], p.pos[1] - a[2]];
    let q = [d[0] * u[0] + d[1] * u[1], d[0] * v[0] + d[1] * v[1]];
    if a[3] > 0.5 {
        hit(p, [-u[0], -u[1]], q[0] - b[2], r);
        hit(p, u, -b[2] - q[0], r);
        hit(p, [-v[0], -v[1]], q[1] - b[3], r);
        hit(p, v, -b[3] - q[1], r);
        return;
    }
    let pen = [b[2] - q[0].abs(), b[3] - q[1].abs()];
    if pen[0] > 0.0 && pen[1] > 0.0 {
        let side = |x: f32| if x >= 0.0 { 1.0 } else { -1.0 };
        match pen[0] < pen[1] {
            true => hit(p, u.map(|c| c * side(q[0])), pen[0], r),
            false => hit(p, v.map(|c| c * side(q[1])), pen[1], r),
        }
    }
}

pub(super) fn crossed(p: &mut Particle, (a, b): ([f32; 4], [f32; 4]), prev: [f32; 2], r: [f32; 4]) {
    let (o, d) = ([a[1], a[2]], [b[0] - a[1], b[1] - a[2]]);
    let cross = |v: [f32; 2]| d[0] * (v[1] - o[1]) - d[1] * (v[0] - o[0]);
    let (s0, s1) = (cross(prev), cross(p.pos));
    let l2 = d[0] * d[0] + d[1] * d[1];
    if s0 * s1 >= 0.0 || l2 < 1e-9 {
        return;
    }
    let t = s0 / (s0 - s1);
    let at = [prev[0] + (p.pos[0] - prev[0]) * t, prev[1] + (p.pos[1] - prev[1]) * t];
    let along = ((at[0] - o[0]) * d[0] + (at[1] - o[1]) * d[1]) / l2;
    if !(0.0..=1.0).contains(&along) {
        return;
    }
    let l = l2.sqrt();
    let k = if s0 < 0.0 { -1.0 } else { 1.0 };
    let n = [-d[1] / l * k, d[0] / l * k];
    let depth = 0.01 - ((p.pos[0] - o[0]) * n[0] + (p.pos[1] - o[1]) * n[1]);
    hit(p, n, depth, r);
}

pub(super) fn shaped(
    p: &mut Particle,
    (a, b, c): ([f32; 4], [f32; 4], [f32; 4]),
    points: &[[f32; 2]],
    r: [f32; 4],
) {
    let q = crate::Affine([b[0], b[1], b[2], b[3], c[0], c[1]]).apply(p.pos);
    let Some((d, g)) = super::field::sample(points, a, [c[2], c[3]], q) else { return };
    let n = [b[0] * g[0] + b[1] * g[1], b[2] * g[0] + b[3] * g[1]];
    let l = (n[0] * n[0] + n[1] * n[1]).sqrt();
    let unit = 1.0 / (b[0] * b[3] - b[1] * b[2]).abs().sqrt().max(1e-6);
    hit(p, if l > 1e-6 { [n[0] / l, n[1] / l] } else { [0.0, -1.0] }, -d * unit, r);
}
