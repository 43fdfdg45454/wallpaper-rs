mod judge;
mod plan;
mod slow;
#[cfg(test)]
mod spans;
mod stamps;
#[cfg(test)]
mod tests;

use crate::Renderer;
pub use judge::set_gpu_budget;
pub(crate) use plan::Sim;
pub(crate) use slow::{charge, charged, pardon, slow};
pub(crate) use stamps::SIM_AT;
use stamps::Stamps;

const WATCH: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Timing {
    Off,
    Stamps(usize),
    Watch,
}

pub(crate) struct Clock {
    stamps: Option<Stamps>,
    queries: bool,
    frame: u64,
    judge: judge::Judge,
    plan: plan::Plan,
    timed: Vec<(u64, bool, f64)>,
    sim: (bool, f64),
    pub(crate) gpu_ms: f64,
    pub(crate) sim_ms: f64,
    pub(crate) fresh: bool,
    pub(crate) samples: u64,
}

impl Clock {
    pub(crate) fn new(r: &Renderer) -> Clock {
        let queries = r.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let judge = judge::Judge::new(r.tier());
        let (plan, timed) = (plan::Plan::new(), Vec::new());
        let (gpu_ms, sim_ms, fresh, samples, sim) = (0.0, 0.0, false, 0, (false, 0.0));
        Clock { stamps: None, queries, frame: 0, judge, plan, timed, sim, gpu_ms, sim_ms, fresh, samples }
    }

    pub(crate) fn start(&mut self, r: &Renderer, ops: &[super::Op], trial: bool, drawn: bool) -> Timing {
        self.plan.marks.clear();
        self.frame += 1;
        let (watched, sim) = (trial || self.judge.suspect(), std::mem::take(&mut self.sim));
        if !self.queries {
            let watch = drawn && watched || self.frame.is_multiple_of(WATCH);
            self.sim = sim;
            return if watch { Timing::Watch } else { Timing::Off };
        }
        let stamps = self.stamps.get_or_insert_with(|| Stamps::new(r));
        let Some(free) = stamps.free() else { return Timing::Off };
        self.plan.plan(ops, self.frame);
        stamps.take(free, &mut self.plan.runs, (self.plan.wide, sim.0));
        Timing::Stamps(free)
    }

    pub(crate) fn writes(&self, op: usize) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let (begin, end) = self.plan.marks.get(op).copied()?;
        let query_set = &self.stamps.as_ref()?.set;
        (begin.is_some() || end.is_some()).then_some(wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        })
    }

    pub(crate) fn open(&self, encoder: &mut wgpu::CommandEncoder, timing: Timing) {
        if let (Timing::Stamps(_), Some(stamps)) = (timing, &self.stamps) {
            stamps.mark(encoder, false);
        }
    }

    pub(crate) fn close(&self, encoder: &mut wgpu::CommandEncoder, timing: Timing) {
        if let (Timing::Stamps(back), Some(stamps)) = (timing, &self.stamps) {
            stamps.close(encoder, back);
        }
    }

    pub(crate) fn collect(&mut self) -> Vec<(u64, String)> {
        let Some(stamps) = self.stamps.as_mut() else { return Vec::new() };
        let (judge, now, mut charged) = (&mut self.judge, std::time::Instant::now(), Vec::new());
        let (gpu, fresh, samples) = (&mut self.gpu_ms, &mut self.fresh, &mut self.samples);
        let sim_ms = &mut self.sim_ms;
        stamps.collect(&mut self.timed, |runs, [whole, sim], wide| {
            (*gpu, *sim_ms, *fresh, *samples) = (whole + sim, sim, true, *samples + 1);
            charged.extend(judge.frame(runs, (Some(whole), wide), now));
        });
        charged
    }

    pub(crate) fn watched(&mut self, keys: &[u64], ms: f64) -> Vec<(u64, String)> {
        self.sim_ms = std::mem::take(&mut self.sim).1;
        (self.gpu_ms, self.fresh, self.samples) = (ms + self.sim_ms, true, self.samples + 1);
        self.judge.watched(keys, ms)
    }
}
