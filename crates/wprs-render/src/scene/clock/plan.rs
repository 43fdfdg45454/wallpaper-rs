use super::super::pipes::Shader;
use super::super::{Fill, Op};
use super::stamps::Stamps;
use super::{Clock, WATCH};
use crate::Renderer;
use std::time::Instant;

pub(super) const SLOTS: usize = 32;

fn timed(op: &Op) -> Option<(u64, bool)> {
    match op {
        Op::Effect(Fill { shader: Shader::Own(key, ..), .. }) => Some((*key, true)),
        Op::Effect(Fill { shader: Shader::Fx(key, _), .. }) => Some((*key, false)),
        Op::Particles { timed, .. } if *timed != 0 => Some((*timed, true)),
        _ => None,
    }
}

pub(super) struct Plan {
    pub(super) marks: Vec<(Option<u32>, Option<u32>)>,
    pub(super) runs: Vec<(u64, bool)>,
    pub(super) wide: bool,
    spans: Vec<(usize, usize, u64, bool)>,
}

impl Plan {
    pub(super) fn new() -> Plan {
        Plan { marks: Vec::new(), runs: Vec::new(), wide: false, spans: Vec::new() }
    }

    pub(super) fn plan(&mut self, ops: &[Op], turn: u64) {
        let Plan { marks, runs, wide, spans } = self;
        marks.clear();
        runs.clear();
        spans.clear();
        marks.resize(ops.len(), (None, None));
        for (k, op) in ops.iter().enumerate() {
            let Some((key, own)) = timed(op) else { continue };
            match spans.last_mut() {
                Some(last) if last.1 + 1 == k && last.2 == key => last.1 = k,
                _ => spans.push((k, k, key, own)),
            }
        }
        let (n, room) = (spans.len(), SLOTS - 1);
        *wide = n > room && spans.iter().any(|s| s.3);
        let first = if n > room { (turn as usize).wrapping_mul(room) % n } else { 0 };
        let wrap = (first + room).saturating_sub(n).min(first);
        let picked = (0..wrap).chain(first..(first + room).min(n));
        for (slot, at) in picked.enumerate() {
            let (begin, end, key, own) = spans[at];
            let slot = slot as u32 + 1;
            marks[begin].0 = Some(slot * 2);
            marks[end].1 = Some(slot * 2 + 1);
            runs.push((key, own));
        }
    }
}

pub(crate) enum Sim {
    Off,
    Stamps(wgpu::QuerySet),
    Watch(Instant),
}

impl Clock {
    pub(crate) fn sim(&mut self, r: &Renderer) -> Sim {
        if !self.queries {
            let watch = (self.frame + 1).is_multiple_of(WATCH) && r.drained();
            return if watch { Sim::Watch(Instant::now()) } else { Sim::Off };
        }
        let stamps = self.stamps.get_or_insert_with(|| Stamps::new(r));
        stamps.free().map_or(Sim::Off, |_| Sim::Stamps(stamps.set.clone()))
    }

    pub(crate) fn simmed(&mut self, r: &Renderer, sim: Sim, stamped: bool) {
        self.sim = match sim {
            Sim::Off => (false, 0.0),
            Sim::Stamps(_) => (stamped, 0.0),
            Sim::Watch(at) => (false, if r.drained() { at.elapsed().as_secs_f64() * 1000.0 } else { 0.0 }),
        };
    }
}
