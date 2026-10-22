use super::{Cue, Force, MAX_EMITTERS, MAX_FORCES, MAX_PREWARM, MAX_RATE, STEP, Swarm, System, Target};
use crate::resolve::Context;
use crate::{Content, Frame, Quality, Scene};
use std::cell::RefCell;

impl Swarm {
    fn plan(&mut self, s: &System, cx: &Context, now: f64, view: [f32; 4], quality: Quality) {
        let p = cx.pointer;
        let cursor = [view[0] + p.smooth[0] * view[2], view[1] + p.smooth[1] * view[3]];
        let aim = |t: &Target| match t {
            Target::Point([x, y]) => [*x, *y, 1.0],
            Target::Follow(_) => [cursor[0], cursor[1], f32::from(u8::from(p.inside))],
        };
        let q = quality.particles();
        let start = self.clock.is_none();
        for (e, em) in s.emitters.iter().take(MAX_EMITTERS).enumerate() {
            let rate = em.rate.get(cx);
            self.rates[e] = if rate > 0.0 { rate.min(MAX_RATE) * q } else { 0.0 };
            let [vx, vy] = [p.velocity[0] * view[2], p.velocity[1] * view[3]].map(|v| v * em.inherit);
            self.base.emitters[2 * e + 1] = em.follow.map_or([0.0; 4], |_| [cursor[0], cursor[1], vx, vy]);
            let Some(b) = &em.burst else { continue };
            let fired = match b.on {
                Some(Cue::Beat) => cx.audio.beat && !self.beat,
                Some(Cue::Click) => p.buttons != 0 && self.buttons == 0,
                None => start && b.every.is_none(),
            };
            self.pending[e] = self.pending[e].saturating_add(if fired { b.count } else { 0 });
        }
        (self.beat, self.buttons) = (cx.audio.beat, p.buttons);
        for (f, force) in s.forces.iter().take(MAX_FORCES).enumerate() {
            let pull = |kind: f32, t: &Target, k: f32| {
                let [x, y, on] = aim(t);
                [kind, x, y, k * on]
            };
            self.base.forces[f] = match force {
                Force::Gravity(g) => {
                    let [x, y] = g.get(cx).0;
                    [1.0, x, y, 0.0]
                }
                Force::Drag(k) => [2.0, k.get(cx), 0.0, 0.0],
                Force::Turbulence { strength, scale } => [3.0, strength.get(cx), *scale, 0.0],
                Force::Attractor { target, strength } => pull(4.0, target, strength.get(cx)),
                Force::Vortex { center, strength } => pull(5.0, center, strength.get(cx)),
                Force::Orbit { center, strength } => pull(6.0, center, strength.get(cx)),
                Force::Noise(k) => [7.0, k.get(cx), 0.0, 0.0],
            };
        }
        self.base.ids[3] = ((s.capacity as f32 * q) as u32).clamp(1, s.capacity.max(1));
        let (n, dt) = match self.clock {
            None => (((s.prewarm.clamp(0.0, MAX_PREWARM) / STEP).round() as usize).max(1), STEP),
            Some(last) => self.span(s, now - last),
        };
        self.clock = Some(now);
        if n > 0 {
            self.steps.clear();
            self.stamp += 1;
            (0..n).for_each(|_| self.step(s, dt));
        }
        cx.note(self.wake(s));
    }

    fn step(&mut self, s: &System, dt: f32) {
        let mut sim = self.base;
        let first = self.base.ids[1];
        (sim.clock[0], sim.clock[1]) = (dt, self.sim as f32);
        let mut total = 0u32;
        for (e, em) in s.emitters.iter().take(MAX_EMITTERS).enumerate() {
            self.owed[e] = (self.owed[e] + self.rates[e] * dt).min(sim.ids[3] as f32);
            let mut n = self.owed[e] as u32;
            self.owed[e] -= n as f32;
            n = n.saturating_add(std::mem::take(&mut self.pending[e]));
            if let Some(every) = em.burst.as_ref().and_then(|b| b.every.map(|t| (b.count, f64::from(t))))
                && self.sim >= self.next[e]
            {
                n = n.saturating_add(every.0);
                let next = self.next[e] + every.1;
                self.next[e] = if next > self.sim { next } else { self.sim + every.1 };
            }
            total = total.saturating_add(n).min(sim.ids[3]);
            sim.births[e] = total;
        }
        sim.births[s.emitters.len().min(MAX_EMITTERS)..].fill(total);
        self.base.ids[1] = first.wrapping_add(total);
        self.base.ids[2] = self.base.ids[2].wrapping_add(1);
        self.sim += f64::from(dt);
        self.born = if total > 0 { self.sim } else { self.born };
        self.steps.push(sim);
    }
}

pub(crate) fn plan(
    swarms: &RefCell<Vec<Swarm>>,
    scene: &Scene,
    cx: &Context,
    frame: &Frame,
    quality: Quality,
    now: f64,
) {
    let mut swarms = swarms.borrow_mut();
    for ((id, s), swarm) in scene.particles.iter().zip(swarms.iter_mut()) {
        let shown =
            |d: &crate::Draw| !d.hidden && matches!(d.content, Content::Particles { system } if system == id);
        if frame.draws.iter().any(shown) {
            swarm.plan(s, cx, now, frame.view, quality);
        }
    }
}
