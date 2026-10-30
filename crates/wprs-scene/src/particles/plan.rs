use super::points::{Room, bare, turn};
use super::tune::tuned;
use super::{
    Cue, EmitShape, Flock, MAX_EMITTERS2, MAX_HATCH, MAX_PREWARM2, MAX_RATE, Moment, Placement, STEP, Swarm,
    System, WARM, WARM_WORK,
};
use crate::resolve::Context;
use crate::{Affine, Content, Draw, Frame, Quality, Scene};
use std::cell::RefCell;

impl Swarm {
    fn plan(&mut self, systems: &[&System], flock: &Flock, cx: &Context, now: f64, d: &Draw, q: Quality) {
        let (root, canvas, view, p) = (systems[flock.system], cx.canvas, cx.view, cx.pointer);
        let layer = flock.layer.as_ref().map(|_| d.layer);
        let lift = d.world.then(Affine::translate([-d.origin[0] * canvas[0], -d.origin[1] * canvas[1]]));
        let world = root.space == Placement::World;
        let (base, to_sim) =
            if world { (lift, Affine::IDENTITY) } else { (Affine::IDENTITY, lift.inverse()) };
        let [x, y] = to_sim.apply([view[0] + p.smooth[0] * view[2], view[1] + p.smooth[1] * view[3]]);
        let cursor = [x, y, f32::from(u8::from(p.inside))];
        let home = to_sim.apply([canvas[0] / 2.0, canvas[1] / 2.0]);
        let drift = bare(to_sim).apply([p.velocity[0] * view[2], p.velocity[1] * view[3]]);
        let tune = tuned(cx, layer, root);
        let share = q.particles() * tune.count;
        self.scale = f64::from(tune.time_scale);
        let mut frames: Vec<Affine> = Vec::with_capacity(flock.parts.len());
        let me = std::ptr::from_ref(self).addr();
        for (k, part) in flock.parts.iter().enumerate() {
            let s = systems[part.system];
            let host = part.host.map(|h| (h, systems[flock.parts[h].system]));
            let link = host.and_then(|(_, h)| h.children.get(part.link));
            let frame = match (host, link) {
                (Some((h, hs)), Some(c)) => {
                    let rel = Affine::translate(c.offset.unwrap_or_default())
                        .then(Affine::rotate(c.angle.unwrap_or(0.0)))
                        .then(Affine::scale([c.scale; 2]));
                    let origin =
                        hs.points.first().and_then(|p| p.at.as_ref()).map_or([0.0; 2], |v| v.get(cx).0);
                    match c.on {
                        Moment::Static => frames[h].then(Affine::translate(origin)).then(rel),
                        _ => bare(frames[h]).then(rel),
                    }
                }
                _ => base,
            };
            frames.push(frame);
            let hatched = link.is_some_and(|c| c.on != Moment::Static);
            let anchor = if hatched { frames[part.host.unwrap_or(0)] } else { frame };
            let lent = link.filter(|c| c.points_from).and(host.map(|(_, h)| h));
            let ends = (cursor, home);
            let room = Room::new(cx, lent.unwrap_or(s), (anchor, to_sim), ends, layer.filter(|_| k == 0));
            let sim = &mut self.frames[k];
            *sim = self.bases[k];
            sim.ids[3] = ((s.capacity as f32 * share) as u32).clamp(1, s.capacity.max(1));
            sim.init[0][0] *= tune.life;
            sim.init[0][1] *= tune.life;
            [2, 3].iter().for_each(|&i| sim.init[0][i] *= tune.speed);
            [0, 1].iter().for_each(|&i| sim.init[4][i] *= tune.speed);
            sim.init[5].iter_mut().for_each(|v| *v *= tune.speed);
            if let Some(c) = &s.init.inherit {
                let [x, y, on] = room.aim(&c.from);
                let key = me + 16 * k;
                let v = match c.from {
                    super::Target::Follow(_) => drift.map(|v| v * on),
                    _ => cx.velocity(key, [x, y]),
                };
                (sim.init[9][0], sim.init[9][1]) = (v[0], v[1]);
            }
            let mut counts = [0u32; MAX_EMITTERS2];
            for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
                let rate = em.rate.get(cx);
                let gated = em.audio_gate && cx.audio.silent;
                let rate = if rate > 0.0 && !gated { rate.min(MAX_RATE) * share } else { 0.0 };
                self.feeds[k][e].rate = rate;
                let at = match &em.shape {
                    EmitShape::Box(a) => a.at,
                    EmitShape::Circle { at, .. } => *at,
                    _ => None,
                };
                let mut place = frame;
                if let Some(h) = at {
                    let n = usize::from(h.0);
                    place = if hatched {
                        let c = s.points.get(n).and_then(|c| c.angle.as_ref());
                        let turned = Affine::rotate(c.map_or(0.0, |a| a.get(cx)));
                        frame.then(Affine::translate(room_local(s, h.0, cx))).then(turned)
                    } else {
                        let [px, py, angle] = room.points[n];
                        let turned = Affine::rotate(angle - turn(frame));
                        Affine::translate([px, py]).then(bare(frame)).then(turned)
                    };
                }
                let mut motion = [place.0[4], place.0[5], 0.0, 0.0];
                if em.follow.is_some() {
                    (motion[0], motion[1]) = (motion[0] + cursor[0], motion[1] + cursor[1]);
                    place = bare(place);
                    (motion[2], motion[3]) = (drift[0] * em.inherit, drift[1] * em.inherit);
                }
                let [a, b, c, d, ..] = place.0;
                sim.emitters[4 * e + 2] = [a, b, c, d];
                sim.emitters[4 * e + 3] = motion;
                sim.rates[e] = rate;
                let once = em.burst.as_ref().filter(|b| b.every.is_none() && b.on.is_none());
                counts[e] = once.map_or(0, |b| b.count);
                let Some(b) = em.burst.as_ref().filter(|_| !hatched && !gated) else { continue };
                let fired = match b.on {
                    Some(Cue::Beat) => cx.audio.beat && !self.beat,
                    Some(Cue::Click) => p.buttons != 0 && self.buttons == 0,
                    None => false,
                };
                let f = &mut self.feeds[k][e];
                f.pending = f.pending.saturating_add(if fired && f.on { b.count } else { 0 });
            }
            if hatched {
                if counts.iter().all(|c| *c == 0) {
                    counts[0] = 1;
                }
                let mut total = 0u32;
                for (e, n) in counts.iter().enumerate() {
                    total = total.saturating_add(*n).min(MAX_HATCH);
                    sim.births[e] = total;
                }
                self.feeds[k].iter_mut().for_each(|f| f.rate = 0.0);
            }
            super::drive::forces(sim, s, &room, cx);
            super::drive::colliders(sim, s, anchor);
        }
        (self.beat, self.buttons) = (cx.audio.beat, p.buttons);
        let due = self.due(systems, flock);
        let (n, dt) = match self.clock {
            None => {
                let warm = root.prewarm.clamp(0.0, MAX_PREWARM2);
                let total: u64 = flock.parts.iter().map(|p| u64::from(systems[p.system].capacity)).sum();
                let cap = (WARM_WORK / total.max(1)).max(1) as usize;
                match warm > 0.0 {
                    true => (((warm / WARM).round() as usize).clamp(1, cap), WARM),
                    false => (1, STEP),
                }
            }
            Some(last) => self.span(due, now - last),
        };
        self.clock = Some(now);
        if n > 0 {
            self.steps.clear();
            self.stamp += 1;
            (0..n).for_each(|_| self.step(systems, flock, dt));
        }
        let due = self.due(systems, flock);
        cx.note(self.wake(due));
    }
}

fn room_local(s: &System, k: u8, cx: &Context) -> [f32; 2] {
    let at = s.points.get(usize::from(k)).and_then(|p| p.at.as_ref());
    at.map_or([0.0; 2], |v| v.get(cx).0)
}

pub(crate) fn plan(
    swarms: &RefCell<Vec<Swarm>>,
    (scene, flocks): (&Scene, &[Flock]),
    cx: &Context,
    frame: &Frame,
    quality: Quality,
    now: f64,
) {
    let systems: Vec<&System> = scene.particles.values().collect();
    let mut swarms = swarms.borrow_mut();
    for (i, (flock, swarm)) in flocks.iter().zip(swarms.iter_mut()).enumerate() {
        let shown =
            |d: &&Draw| !d.hidden && matches!(d.content, Content::Particles { flock: f, .. } if f == i);
        if let Some(d) = frame.draws.iter().find(shown) {
            swarm.plan(&systems, flock, cx, now, d, quality);
        }
    }
}
