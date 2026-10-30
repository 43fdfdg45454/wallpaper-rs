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
pub(crate) use slow::{charge, charged, pardon, slow};
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
    timed: Vec<(u64, f64)>,
}

impl Clock {
    pub(crate) fn new(r: &Renderer) -> Clock {
        let queries = r.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let judge = judge::Judge::new(r.tier());
        Clock { stamps: None, queries, frame: 0, judge, plan: plan::Plan::new(), timed: Vec::new() }
    }

    pub(crate) fn start(&mut self, r: &Renderer, ops: &[super::Op], trial: bool, drawn: bool) -> Timing {
        self.plan.marks.clear();
        if !drawn {
            return Timing::Off;
        }
        self.frame += 1;
        let watched = trial || self.judge.suspect();
        if !self.queries {
            return if watched || self.frame.is_multiple_of(WATCH) { Timing::Watch } else { Timing::Off };
        }
        let stamps = self.stamps.get_or_insert_with(|| Stamps::new(r));
        let Some(free) = stamps.free() else { return Timing::Off };
        self.plan.plan(ops, self.frame);
        if self.plan.runs.is_empty() {
            self.plan.marks.clear();
            return Timing::Off;
        }
        stamps.take(free, &mut self.plan.runs, self.plan.whole);
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

    pub(crate) fn close(&self, encoder: &mut wgpu::CommandEncoder, timing: Timing) {
        if let (Timing::Stamps(back), Some(stamps)) = (timing, &self.stamps) {
            stamps.close(encoder, back);
        }
    }

    pub(crate) fn collect(&mut self) -> Vec<(u64, String)> {
        let Some(stamps) = self.stamps.as_mut() else { return Vec::new() };
        let (judge, now, mut charged) = (&mut self.judge, std::time::Instant::now(), Vec::new());
        stamps.collect(&mut self.timed, |runs, whole| charged.extend(judge.frame(runs, whole, now)));
        charged
    }

    pub(crate) fn watched(&mut self, keys: &[u64], ms: f64) -> Vec<(u64, String)> {
        self.judge.watched(keys, ms)
    }
}
