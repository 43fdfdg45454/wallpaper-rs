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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stamping {
    #[default]
    All,
    Frame,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Timing {
    Off,
    Stamps(usize),
    Watch,
}

pub(crate) struct Clock {
    stamps: Option<Stamps>,
    queries: bool,
    pub(crate) stamping: Stamping,
    frame: u64,
    judge: judge::Judge,
    plan: plan::Plan,
    timed: Vec<(u64, bool, f64)>,
    sim: (bool, f64),
    pub(crate) gpu_ms: f64,
    pub(crate) sim_ms: f64,
    pub(crate) span_ms: f64,
    pub(crate) fresh: bool,
    pub(crate) samples: u64,
    edge: u64,
}

impl Clock {
    pub(crate) fn new(r: &Renderer) -> Clock {
        let queries = r.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let judge = judge::Judge::new(r.tier());
        let (plan, timed) = (plan::Plan::new(), Vec::new());
        let (gpu_ms, sim_ms, fresh, samples, sim) = (0.0, 0.0, false, 0, (false, 0.0));
        let stamping = Stamping::All;
        Clock {
            stamps: None,
            queries,
            stamping,
            frame: 0,
            judge,
            plan,
            timed,
            sim,
            gpu_ms,
            sim_ms,
            span_ms: 0.0,
            fresh,
            samples,
            edge: 0,
        }
    }

    pub(crate) fn start(&mut self, r: &Renderer, ops: &[super::Op], trial: bool, drawn: bool) -> Timing {
        self.plan.marks.clear();
        self.frame += 1;
        let (watched, sim) = (trial || self.judge.suspect(), std::mem::take(&mut self.sim));
        if self.stamping == Stamping::Off {
            return Timing::Off;
        }
        if !self.queries {
            let watch = drawn && watched || self.frame.is_multiple_of(WATCH);
            self.sim = sim;
            return if watch { Timing::Watch } else { Timing::Off };
        }
        let stamps = self.stamps.get_or_insert_with(|| Stamps::new(r));
        let Some(free) = stamps.free() else { return Timing::Off };
        if self.stamping == Stamping::All {
            self.plan.plan(ops, self.frame);
        } else {
            self.plan.runs.clear();
            self.plan.wide = false;
        }
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
        let (sim_ms, span, edge, period) =
            (&mut self.sim_ms, &mut self.span_ms, &mut self.edge, stamps.period);
        stamps.collect(&mut self.timed, |runs, [whole, sim], wide| {
            let ms = |ticks: u64| ticks as f64 * period / 1e6;
            let ticks;
            (ticks, *edge) = stamps::busy([whole, sim], *edge);
            let (busy, sim) = (ms(ticks), ms(sim.1.saturating_sub(sim.0)));
            (*span, *sim_ms) = (ms(whole.1.saturating_sub(whole.0)) + sim, sim);
            if ticks > 0 {
                (*gpu, *fresh, *samples) = (busy, true, *samples + 1);
            }
            charged.extend(judge.frame(runs, ((ticks > 0).then_some(busy), wide), now));
        });
        charged
    }

    pub(crate) fn watched(&mut self, keys: &[u64], ms: f64) -> Vec<(u64, String)> {
        self.sim_ms = std::mem::take(&mut self.sim).1;
        (self.gpu_ms, self.fresh, self.samples) = (ms + self.sim_ms, true, self.samples + 1);
        self.span_ms = self.gpu_ms;
        self.judge.watched(keys, ms)
    }
}
