use super::super::pipes::Shader;
use super::super::{Fill, Op};

pub(super) const SLOTS: usize = 32;

fn own(op: &Op) -> Option<u64> {
    match op {
        Op::Effect(Fill { shader: Shader::Own(key, ..), .. }) => Some(*key),
        _ => None,
    }
}

pub(super) struct Plan {
    pub(super) marks: Vec<(Option<u32>, Option<u32>)>,
    pub(super) runs: Vec<u64>,
    pub(super) whole: bool,
    spans: Vec<(usize, usize, u64)>,
}

impl Plan {
    pub(super) fn new() -> Plan {
        Plan { marks: Vec::new(), runs: Vec::new(), whole: false, spans: Vec::new() }
    }

    pub(super) fn plan(&mut self, ops: &[Op], turn: u64) {
        let Plan { marks, runs, whole, spans } = self;
        marks.clear();
        runs.clear();
        spans.clear();
        marks.resize(ops.len(), (None, None));
        for (k, op) in ops.iter().enumerate() {
            let Some(key) = own(op) else { continue };
            match spans.last_mut() {
                Some(last) if last.1 + 1 == k && last.2 == key => last.1 = k,
                _ => spans.push((k, k, key)),
            }
        }
        let n = spans.len();
        let wide = n > SLOTS;
        *whole = wide;
        let lo = usize::from(wide);
        let (m, take) = (n - 2 * lo, SLOTS - 2 * lo);
        let first = if wide { (turn as usize).wrapping_mul(take) % m } else { 0 };
        let wrap = (first + take).saturating_sub(m).min(first);
        let last = (first + take).min(m);
        let rotated = (0..wrap).chain(first..last).map(|k| k + lo);
        let ends = |at: usize| wide.then_some(at);
        for (slot, at) in ends(0).into_iter().chain(rotated).chain(ends(n.wrapping_sub(1))).enumerate() {
            let (begin, end, key) = spans[at];
            marks[begin].0 = Some(slot as u32 * 2);
            marks[end].1 = Some(slot as u32 * 2 + 1);
            runs.push(key);
        }
    }
}
