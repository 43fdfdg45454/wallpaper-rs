use super::points::{Room, bare};
use super::{Falloff, Force, MAX_COLLIDERS2, MAX_FORCES2, Sim, Surface, System, Target, Wobble};
use crate::Affine;
use crate::resolve::Context;

const NONE: [f32; 4] = [0.0; 4];

fn fall(f: Option<Falloff>, default: f32) -> f32 {
    f.map_or(default, |f| match f {
        Falloff::Constant => 0.0,
        Falloff::Linear => 1.0,
        Falloff::InverseSquare => 2.0,
    })
}

pub(super) fn forces(sim: &mut Sim, s: &System, room: &Room, cx: &Context) {
    for (k, o) in s.forces.iter().take(MAX_FORCES2).enumerate() {
        let pull = |kind: f32, t: &Target, v: f32| {
            let [x, y, on] = room.aim(t);
            [kind, x, y, v * on]
        };
        let reach = |r: Option<f32>, f: Option<Falloff>, d: f32| [r.unwrap_or(0.0), fall(f, d), 0.0, -1.0];
        let (head, a, b) = match &o.force {
            Force::Gravity(g) => {
                let [x, y] = g.get(cx).0;
                ([1.0, x, y, 0.0], NONE, NONE)
            }
            Force::Drag(k) => ([2.0, k.get(cx), 0.0, 0.0], NONE, NONE),
            Force::Turbulence { strength, scale, speed, time_scale, mask } => {
                let [mx, my] = mask.unwrap_or([1.0; 2]);
                let drift = [speed.unwrap_or(0.0), 0.2 * time_scale.unwrap_or(1.0), mx, my];
                ([3.0, strength.get(cx), *scale, 0.0], drift, NONE)
            }
            Force::Attractor { target, strength, radius, falloff } => {
                (pull(4.0, target, strength.get(cx)), reach(*radius, *falloff, 2.0), NONE)
            }
            Force::Vortex { center, strength, radius, falloff, inner, outer, ring } => {
                let mut r = reach(*radius, *falloff, 1.0);
                (r[2], r[3]) = (inner.unwrap_or(0.0), outer.or(*inner).unwrap_or(-1.0));
                let ring = ring.map_or(NONE, |g| [g.radius, g.width, g.pull, 1.0]);
                (pull(5.0, center, strength.get(cx)), r, ring)
            }
            Force::Orbit { center, strength, radius, falloff } => {
                (pull(6.0, center, strength.get(cx)), reach(*radius, *falloff, 0.0), NONE)
            }
            Force::Noise(k) => ([7.0, k.get(cx), 0.0, 0.0], NONE, NONE),
            Force::Oscillate(w) => {
                let what = match w.what {
                    Wobble::Position => 0.0,
                    Wobble::Alpha => 1.0,
                    Wobble::Size => 2.0,
                };
                let [mx, my] = w.mask.unwrap_or([1.0; 2]);
                let (f, g) =
                    ([w.freq[0], w.freq[1], w.scale[0], w.scale[1]], [w.phase[0], w.phase[1], mx, my]);
                ([8.0, what, 0.0, 0.0], f, g)
            }
            Force::Cap(v) => ([9.0, v.get(cx), 0.0, 0.0], NONE, NONE),
            Force::Torque(v) => ([10.0, v.get(cx), 0.0, 0.0], NONE, NONE),
            Force::SpinDrag(v) => ([11.0, v.get(cx), 0.0, 0.0], NONE, NONE),
            Force::SlowNear(n) => (pull(12.0, &n.target, 1.0), [n.inner, n.outer, n.factor, 0.0], NONE),
            _ => (NONE, NONE, NONE),
        };
        let window = o.window.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        sim.forces[4 * k..4 * k + 4].copy_from_slice(&[head, a, b, window]);
    }
}

pub(super) fn colliders(sim: &mut Sim, s: &System, frame: Affine) {
    let grow = (frame.0[0] * frame.0[3] - frame.0[1] * frame.0[2]).abs().sqrt();
    for (k, c) in s.collide.iter().take(MAX_COLLIDERS2).enumerate() {
        let ([kind, x, y, z], w) = match &c.bounds {
            Surface::Canvas => ([1.0, 0.0, 0.0, 0.0], 0.0),
            Surface::Plane { point, normal } => {
                let ([x, y], [u, v]) = (frame.apply(*point), bare(frame).apply(*normal));
                let l = u.hypot(v).max(1e-6);
                ([2.0, x, y, u / l], v / l)
            }
            Surface::Circle { center, radius, .. } => {
                let [x, y] = frame.apply(*center);
                ([3.0, x, y, radius * grow], 0.0)
            }
            Surface::Mask { .. } => ([4.0, 0.0, 0.0, 0.0], 0.0),
            Surface::Box { .. } | Surface::Segment { .. } | Surface::Layer { .. } => (NONE, 0.0),
        };
        sim.colliders[2 * k] = [kind, x, y, z];
        sim.colliders[2 * k + 1] = [w, c.bounce, c.friction, f32::from(u8::from(c.kill))];
    }
}
