use super::Puppet;
use super::limb::{angle, end, gap, length, mirrored, pivot, range, turn, wrap};
use super::swing::{Body, MAX_STEPS, QUIET, QUIET_STEPS, Sim};
use crate::resolve::Context;
use crate::rigged::math::{Trs, invert, mul};
use crate::{Body as Mode, BonePhysics, MeshLayer, Wake};

fn mix(a: [f32; 2], b: [f32; 2], f: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

fn advance(b: &mut Body, e: &BonePhysics, sim: &Sim, (f0, f1): (f32, f32)) {
    let h = sim.h;
    let lerp = |b: &Body, i: usize, f: f32| mix(b.before[i], b.pins[i], f);
    let decay = (-e.damping * 10.0 * h).exp();
    let k = match e.mode {
        Mode::Spring => (e.stiffness * 20.0).powi(2),
        Mode::Rigid | Mode::Rope => (e.stiffness * 4.0).powi(2),
    };
    let (pin, from) = (lerp(b, 0, f1), lerp(b, 0, f0));
    let carry = gap(from, pin).map(|v| v * (1.0 - e.inertia));
    let pull = |at: [f32; 2], was: [f32; 2], target: [f32; 2], drop: f32, i: usize| {
        let v = gap(was, at).map(|v| v * decay);
        let gust = 0.65 + 0.35 * crate::camera::noise(sim.t * 0.9 + i as f32 * 0.37, 7);
        let air = [0, 1].map(|c| (sim.wind[c] * gust - v[c] / h) * 4.0 * e.wind);
        let a = [k * (target[0] - at[0]) + air[0], k * (target[1] - at[1]) + air[1] + drop];
        [0, 1].map(|c| at[c] + v[c] + a[c] * h * h)
    };
    let mut speed = 0.0f32;
    let drop = -e.gravity * 98.0;
    match e.translation {
        None => (b.at[0], b.was[0]) = (pin, pin),
        Some([lo, hi]) => {
            let next = pull(b.at[0], b.was[0], pin, drop, 0);
            let (dx, dy) = ((next[0] - pin[0]).clamp(lo, hi), (pin[1] - next[1]).clamp(lo, hi));
            b.was[0] = b.at[0];
            b.at[0] = [pin[0] + dx, pin[1] - dy];
            speed = length(gap(b.was[0], b.at[0]));
        }
    }
    for i in 1..b.at.len() {
        for p in [&mut b.at[i], &mut b.was[i]] {
            *p = [p[0] + carry[0], p[1] + carry[1]];
        }
        let next = pull(b.at[i], b.was[i], lerp(b, i, f1), drop, i);
        b.was[i] = b.at[i];
        b.at[i] = next;
    }
    let limit = range(e.rotation);
    for i in 1..b.at.len() {
        let home = gap(lerp(b, i - 1, f1), lerp(b, i, f1));
        let rest = length(home);
        let mut a = angle(gap(b.at[i - 1], b.at[i]));
        if let Some([lo, hi]) = limit {
            a = angle(home) + wrap(a - angle(home)).clamp(lo, hi);
        }
        b.at[i] = [b.at[i - 1][0] + rest * a.cos(), b.at[i - 1][1] + rest * a.sin()];
        speed = speed.max(length(gap(b.was[i], b.at[i])));
    }
    b.quiet = if speed < QUIET { (b.quiet + 1).min(QUIET_STEPS) } else { 0 };
}

impl Puppet {
    pub(crate) fn swing(&mut self, cx: &Context, m: &MeshLayer, sim: &Sim) {
        let steps = ((self.lag / sim.h) as u32).min(MAX_STEPS);
        self.lag = if steps == MAX_STEPS || sim.frozen { 0.0 } else { self.lag - steps as f32 * sim.h };
        let ran = sim.h * steps as f32;
        let span = if steps == MAX_STEPS { ran } else { self.since.max(ran) }.max(1e-6);
        let at = |s: u32| (s as f32 * sim.h / span).min(1.0);
        if steps > 0 || sim.frozen {
            self.since = self.lag;
        }
        for k in 0..self.bodies.len() {
            let e = &m.rig.physics[self.bodies[k].entry];
            let body = &mut self.bodies[k];
            body.pin(&self.worlds);
            if sim.frozen {
                body.hold();
                continue;
            }
            if body.before != body.pins {
                body.quiet = 0;
            }
            let windy = e.wind > 0.0 && sim.wind != [0.0; 2];
            let busy = windy || body.quiet < QUIET_STEPS;
            if busy {
                (0..steps).for_each(|s| advance(body, e, sim, (at(s), at(s + 1))));
            }
            if steps > 0 {
                let f = at(steps);
                body.before.iter_mut().zip(&body.pins).for_each(|(b, p)| *b = mix(*b, *p, f));
            }
            if windy || body.quiet < QUIET_STEPS {
                cx.note(Wake::Frame);
            }
            self.lean(k, e, if sim.frozen { 0.0 } else { (self.lag / sim.h).min(1.0) });
            self.worlds(cx, m);
        }
    }

    fn lean(&mut self, k: usize, e: &BonePhysics, ahead: f32) {
        let Puppet { bodies, locals, worlds, rig, .. } = self;
        let body = &bodies[k];
        let shown = |i: usize| mix(body.was[i], body.at[i], 1.0 + ahead);
        for (i, &n) in body.nodes.iter().enumerate() {
            let n = n as usize;
            let parent = rig.joints[n].parent.map(|p| worlds[p as usize]);
            let world = |l: &Trs| parent.map_or(l.matrix(), |p| mul(&p, &l.matrix()));
            if i == 0 && e.translation.is_some() {
                let [x, y] = gap(pivot(&world(&locals[n])), shown(0));
                let back = parent.map_or(crate::rigged::math::IDENTITY, |p| invert(&p));
                (0..3).for_each(|r| locals[n].t[r] += back[r * 4] * x + back[r * 4 + 1] * y);
            }
            let w = world(&locals[n]);
            let p = pivot(&w);
            let delta = wrap(angle(gap(p, shown(i + 1))) - angle(gap(p, end(&w, body.tips[i]))));
            turn(&mut locals[n], delta * mirrored(parent.as_ref()));
            worlds[n] = world(&locals[n]);
        }
    }
}
