use super::aim::Aim;
use super::fit::{Fit, fit as fit_into};
use super::points::{Room, bare};
use super::tune::tuned;
use super::{Flock, MAX_EMITTERS2, MAX_PREWARM2, Moment, Placement, STEP, Swarm, System, WARM};
use crate::resolve::Context;
use crate::{Affine, Content, Draw, Frame, Quality};
use std::cell::RefCell;

impl Swarm {
    fn plan(
        &mut self,
        (systems, flock): (&[System], &Flock),
        cx: &Context,
        now: f64,
        (d, all): (&Draw, &Frame),
        q: Quality,
    ) {
        let (root, canvas, view, p) = (&systems[flock.system], cx.canvas, cx.view, cx.pointer);
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
        let mut frames = std::mem::take(&mut self.places);
        frames.clear();
        let me = std::ptr::from_ref(self).addr();
        let dt = self.clock.map_or(0.0, |c| (now - c) as f32);
        self.moves.resize(flock.parts.len(), [[0.0; 6]; MAX_EMITTERS2]);
        for (k, part) in flock.parts.iter().enumerate() {
            let s = &systems[part.system];
            let host = part.host.map(|h| (h, &systems[flock.parts[h].system]));
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
            super::bound::bound(sim, s, cx);
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
            let aim = Aim { frame, room: &room, hatched, cursor, drift, share };
            self.aim(k, s, cx, &aim);
            let fit = Fit { frame, to_sim, room: &room, hatched, draws: all, dt };
            fit_into(&mut self.frames[k], s, &fit, cx, &mut self.moves[k]);
            let sim = &mut self.frames[k];
            super::drive::forces(sim, s, &room, cx);
            super::walls::colliders(sim, s, anchor);
            super::walls::layered(sim, s, all, to_sim);
            sim.flock[2] = q as u32 + 1;
        }
        self.places = frames;
        (self.beat, self.buttons) = (cx.audio.beat, p.buttons);
        let due = self.due(systems, flock);
        let (n, dt) = match self.clock {
            None => {
                let warm = root.prewarm.clamp(0.0, MAX_PREWARM2);
                match warm > 0.0 {
                    true => (((warm / WARM).round() as usize).clamp(1, self.warm), WARM),
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

pub(crate) fn plan(
    swarms: &RefCell<Vec<Swarm>>,
    (systems, flocks): (&[System], &[Flock]),
    cx: &Context,
    frame: &Frame,
    quality: Quality,
    now: f64,
) {
    let mut swarms = swarms.borrow_mut();
    for (i, (flock, swarm)) in flocks.iter().zip(swarms.iter_mut()).enumerate() {
        let shown =
            |d: &&Draw| !d.hidden && matches!(d.content, Content::Particles { flock: f, .. } if f == i);
        if let Some(d) = frame.draws.iter().find(shown) {
            swarm.plan((systems, flock), cx, now, (d, frame), quality);
        }
    }
}
