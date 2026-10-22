use super::{MAX_EMITTERS, Sim, System};
use crate::Wake;

pub const STEP: f32 = 1.0 / 60.0;
pub const SUBSTEPS: usize = 15;
const LONGEST: f64 = 1.0;

#[derive(Debug, Clone, Default)]
pub struct Swarm {
    pub epoch: u32,
    pub stamp: u64,
    pub steps: Vec<Sim>,
    pub(super) base: Sim,
    pub(super) rates: [f32; MAX_EMITTERS],
    pub(super) clock: Option<f64>,
    acc: f64,
    pub(super) sim: f64,
    pub(super) born: f64,
    pub(super) owed: [f32; MAX_EMITTERS],
    pub(super) pending: [u32; MAX_EMITTERS],
    pub(super) next: [f64; MAX_EMITTERS],
    pub(super) beat: bool,
    pub(super) buttons: u8,
}

impl Swarm {
    pub(crate) fn new(s: &System, seed: u64, canvas: [u32; 2]) -> Swarm {
        let mut swarm = Swarm { base: Sim::of(s, seed, canvas), ..Swarm::default() };
        swarm.restart();
        swarm.epoch = 0;
        swarm
    }

    pub(crate) fn restart(&mut self) {
        let (epoch, mut base) = (self.epoch.wrapping_add(1), self.base);
        (base.ids[1], base.ids[2]) = (0, 0);
        *self = Swarm { epoch, stamp: self.stamp, base, born: f64::NEG_INFINITY, ..Swarm::default() };
    }

    pub fn alive(&self) -> bool {
        self.sim < self.born + f64::from(self.base.init[0][1] + STEP)
    }

    fn idle(&self) -> bool {
        !self.alive() && self.rates.iter().all(|r| *r <= 0.0) && self.pending.iter().all(|n| *n == 0)
    }

    fn due(&self, s: &System) -> f64 {
        let timed = s
            .emitters
            .iter()
            .zip(&self.next)
            .filter(|(e, _)| e.burst.as_ref().is_some_and(|b| b.every.is_some()));
        timed.fold(f64::INFINITY, |due, (_, next)| due.min(*next))
    }

    pub(super) fn span(&mut self, s: &System, gap: f64) -> (usize, f32) {
        let mut gap = gap.max(0.0);
        if self.idle() {
            let skip = gap.min((self.due(s) - self.sim).max(0.0));
            (self.sim, gap) = (self.sim + skip, gap - skip);
        }
        self.acc += gap * 60.0;
        if self.idle() && self.sim >= self.due(s) {
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

    pub(super) fn wake(&self, s: &System) -> Wake {
        match self.due(s) {
            _ if !self.idle() => Wake::Frame,
            due if due.is_finite() => Wake::At((due - self.sim).max(0.0)),
            _ => Wake::Idle,
        }
    }
}
