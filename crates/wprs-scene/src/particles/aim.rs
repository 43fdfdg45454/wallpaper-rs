use super::points::{Room, bare, turn};
use super::{Cue, EmitShape, MAX_EMITTERS2, MAX_HATCH, MAX_RATE, Swarm, System};
use crate::Affine;
use crate::resolve::Context;

pub(super) struct Aim<'a> {
    pub frame: Affine,
    pub room: &'a Room,
    pub hatched: bool,
    pub cursor: [f32; 3],
    pub drift: [f32; 2],
    pub share: f32,
}

impl Swarm {
    pub(super) fn aim(&mut self, k: usize, s: &System, cx: &Context, aim: &Aim) {
        let Aim { frame, room, hatched, cursor, drift, share } = *aim;
        let (p, sim) = (cx.pointer, &mut self.frames[k]);
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
    }
}

fn room_local(s: &System, k: u8, cx: &Context) -> [f32; 2] {
    let at = s.points.get(usize::from(k)).and_then(|p| p.at.as_ref());
    at.map_or([0.0; 2], |v| v.get(cx).0)
}
