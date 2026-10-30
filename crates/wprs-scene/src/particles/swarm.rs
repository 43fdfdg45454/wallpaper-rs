use super::{Emitter, Flock, MAX_EMITTERS2, Part, Sim, System};
use crate::Wake;
use crate::particles::motion::rand;

pub const STEP: f32 = 1.0 / 60.0;
pub const WARM: f32 = 1.0 / 15.0;
pub const SUBSTEPS: usize = 15;
pub const WARM_WORK: u64 = 30_000_000;
const LONGEST: f64 = 1.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Feed {
    pub rate: f32,
    pub owed: f32,
    pub pending: u32,
    pub next: f64,
    pub flip: f64,
    pub off: bool,
    pub on: bool,
    pub window: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Play {
    #[default]
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Default)]
pub struct Swarm {
    pub epoch: u32,
    pub stamp: u64,
    pub parts: usize,
    pub steps: Vec<Sim>,
    pub play: Play,
    pub(super) bases: Vec<Sim>,
    pub(super) frames: Vec<Sim>,
    pub(super) feeds: Vec<[Feed; MAX_EMITTERS2]>,
    pub(super) hosts: Vec<Option<usize>>,
    pub(super) clock: Option<f64>,
    acc: f64,
    pub(super) sim: f64,
    pub(super) start: f64,
    pub(super) born: Vec<f64>,
    pub(super) beat: bool,
    pub(super) buttons: u8,
    pub(super) emits: u32,
    pub(super) scale: f64,
}

impl Swarm {
    pub(crate) fn new(systems: &[&System], flock: &Flock, seed: u64, canvas: [u32; 2]) -> Swarm {
        let host = |p: &Part| p.host.map(|h| systems[flock.parts[h].system]);
        let bases: Vec<Sim> =
            flock.parts.iter().map(|p| Sim::of(systems[p.system], p, host(p), seed, canvas)).collect();
        let n = bases.len();
        let hosts = flock.parts.iter().map(|p| p.host).collect();
        let mut swarm = Swarm { parts: n, frames: bases.clone(), bases, hosts, ..Swarm::default() };
        swarm.restart();
        swarm.epoch = 0;
        swarm
    }

    pub(crate) fn restart(&mut self) {
        let epoch = self.epoch.wrapping_add(1);
        let mut bases = std::mem::take(&mut self.bases);
        bases.iter_mut().for_each(|b| (b.ids[1], b.ids[2]) = (0, 0));
        let (n, hosts, frames) =
            (bases.len(), std::mem::take(&mut self.hosts), std::mem::take(&mut self.frames));
        *self = Swarm {
            epoch,
            stamp: self.stamp,
            parts: n,
            bases,
            frames,
            hosts,
            feeds: vec![[Feed::default(); MAX_EMITTERS2]; n],
            born: vec![f64::NEG_INFINITY; n],
            scale: 1.0,
            ..Swarm::default()
        };
    }

    pub fn count(&self) -> usize {
        self.steps.len() / self.parts.max(1)
    }

    pub fn sim(&self, step: usize, part: usize) -> &Sim {
        &self.steps[step * self.parts + part]
    }

    fn until(&self, p: usize) -> f64 {
        let life = f64::from(self.frames[p].init[0][1] + STEP);
        match self.hosts[p].filter(|_| self.frames[p].mode[0] != 0) {
            Some(h) => self.until(h) + life,
            None => self.born[p] + life,
        }
    }

    pub fn alive(&self) -> bool {
        (0..self.parts).any(|p| self.sim < self.until(p))
    }

    fn idle(&self) -> bool {
        let feeding = self.feeds.iter().flatten().any(|f| f.on && (f.rate > 0.0 || f.pending > 0));
        !self.alive() && !feeding && self.emits == 0
    }

    pub(super) fn due(&self, systems: &[&System], flock: &Flock) -> f64 {
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
