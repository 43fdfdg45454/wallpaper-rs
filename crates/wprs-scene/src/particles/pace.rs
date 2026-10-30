use super::motion::rand;
use super::swarm::Play;
use super::{Emitter, Flock, MAX_EMITTERS2, STEP, SUBSTEPS, Sim, Swarm, System};
use crate::Wake;

const LONGEST: f64 = 1.0;

impl Swarm {
    pub(super) fn outlive(&mut self, k: usize, sim: &Sim, born: bool) {
        let life = f64::from(sim.init[0][0].max(sim.init[0][1]).max(0.0) + STEP);
        let from = match self.hosts[k].filter(|_| sim.mode[0] != 0) {
            Some(h) => Some(self.death[h]).filter(|d| self.sim < *d),
            None => born.then_some(self.sim + f64::from(sim.clock[0])),
        };
        if let Some(t) = from {
            self.death[k] = self.death[k].max(t + life);
        }
    }

    pub fn alive(&self) -> bool {
        self.death.iter().any(|d| self.sim < *d)
    }

    fn idle(&self) -> bool {
        let feeding = self.feeds.iter().flatten().any(|f| f.on && (f.rate > 0.0 || f.pending > 0));
        !self.alive() && !feeding && self.emits == 0
    }

    pub(super) fn due(&self, systems: &[System], flock: &Flock) -> f64 {
        let mut due = f64::INFINITY;
        for (p, part) in flock.parts.iter().enumerate().filter(|(p, _)| self.frames[*p].mode[0] == 0) {
            for (e, em) in systems[part.system].emitters.iter().take(MAX_EMITTERS2).enumerate() {
                let f = &self.feeds[p][e];
                let start = self.start + f64::from(em.delay.unwrap_or(0.0));
                let end = start + em.duration.filter(|d| *d > 0.0).map_or(f64::INFINITY, f64::from);
                due = due.min(match () {
                    _ if self.play != Play::Playing => f64::INFINITY,
                    _ if self.sim < start => start,
                    _ if f.off => f.flip,
                    _ if !f.on && self.sim < end => self.sim,
                    _ if f.on && em.burst.as_ref().is_some_and(|b| b.every.is_some()) => f.next,
                    _ => f64::INFINITY,
                });
            }
        }
        due
    }

    pub(super) fn span(&mut self, due: f64, gap: f64) -> (usize, f32) {
        let mut gap = gap.max(0.0) * self.scale;
        if self.idle() {
            let skip = gap.min((due - self.sim).max(0.0));
            (self.sim, gap) = (self.sim + skip, gap - skip);
        }
        self.acc += gap * 60.0;
        if self.idle() && self.sim >= due {
            self.acc = self.acc.max(1.0);
        }
        let n = (self.acc + 1e-6).max(0.0) as usize;
        if n <= SUBSTEPS {
            self.acc -= n as f64;
            return (n, STEP);
        }
        let long = (self.acc / 60.0).min(LONGEST);
        self.acc = 0.0;
        (SUBSTEPS, (long / SUBSTEPS as f64) as f32)
    }

    pub(super) fn wake(&self, due: f64) -> Wake {
        match due {
            _ if !self.idle() => Wake::Frame,
            due if due.is_finite() => Wake::At(((due - self.sim) / self.scale.max(1e-3)).max(0.0)),
            _ => Wake::Idle,
        }
    }

    pub(super) fn timing(&mut self, p: usize, e: usize, em: &Emitter) {
        let (seed, t, start) = (self.bases[p].ids[0], self.sim, self.start);
        let f = &mut self.feeds[p][e];
        let begin = start + f64::from(em.delay.unwrap_or(0.0));
        let length = em.duration.filter(|d| *d > 0.0).map_or(f64::INFINITY, f64::from);
        let pick =
            |w: u32, [lo, hi]: [f32; 2]| f64::from(lo + (hi - lo) * rand(seed, 0xffff_0000 + e as u32, w));
        if let Some(w) = em.periodic {
            if t >= begin && f.flip <= begin {
                (f.flip, f.off, f.window) = (begin + pick(0, w.on), false, 0);
            }
            while t >= f.flip && f.flip > begin {
                (f.off, f.window) = (!f.off, f.window + 1);
                f.flip += pick(f.window, if f.off { w.off } else { w.on }).max(1.0 / 60.0);
            }
        }
        let on = self.play == Play::Playing && t >= begin && t < begin + length && !f.off;
        if on && !f.on {
            let once = em.burst.as_ref().filter(|b| b.every.is_none() && b.on.is_none());
            f.pending = f.pending.saturating_add(once.map_or(0, |b| b.count));
            f.next = t;
        }
        f.on = on;
    }
}
