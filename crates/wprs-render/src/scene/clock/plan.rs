use super::super::pipes::Shader;
use super::super::{Fill, Op};

pub(super) const SLOTS: usize = 32;

fn timed(op: &Op) -> Option<(u64, bool)> {
    match op {
        Op::Effect(Fill { shader: Shader::Own(key, ..), .. }) => Some((*key, true)),
        Op::Effect(Fill { shader: Shader::Fx(key, _), .. }) => Some((*key, false)),
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
