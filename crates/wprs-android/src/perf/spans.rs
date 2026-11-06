use super::{WINDOW_NS, log, monotonic, on};
use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

const KEPT: usize = 256;

static VISIBLE: Mutex<[u32; 2]> = Mutex::new([0; 2]);

#[derive(Default)]
pub(super) struct Spans(VecDeque<(i64, Option<i64>)>);

impl Spans {
    fn mark(&mut self, visible: bool, at: i64) {
        let open = self.0.back().is_some_and(|span| span.1.is_none());
        match (visible, open, self.0.back_mut()) {
            (true, false, _) => self.0.push_back((at, None)),
            (false, true, Some(last)) => last.1 = Some(at),
            _ => {}
        }
        let old = |span: &(i64, Option<i64>)| span.1.is_some_and(|end| at - end > WINDOW_NS);
        while self.0.len() > KEPT || self.0.front().is_some_and(old) {
            self.0.pop_front();
        }
    }

    pub(super) fn seconds(&self, now: i64) -> f64 {
        let from = now - WINDOW_NS;
        let inside =
            |&(start, end): &(i64, Option<i64>)| (end.unwrap_or(now).min(now) - start.max(from)).max(0);
        self.0.iter().map(inside).sum::<i64>() as f64 / 1e9
    }

    pub(super) fn last(&self, now: i64) -> Option<i64> {
        self.0.back().map(|span| span.1.unwrap_or(now).min(now))
    }
}

fn counts<T>(with: impl FnOnce(&mut [u32; 2]) -> T) -> T {
    with(&mut VISIBLE.lock().unwrap_or_else(PoisonError::into_inner))
}

pub(crate) fn show(lock: bool, visible: bool) {
    let tag = usize::from(lock);
    let shown = counts(|counts| {
        counts[tag] = if visible { counts[tag] + 1 } else { counts[tag].saturating_sub(1) };
        counts[tag]
    });
    if on() {
        mark(tag, shown > 0, monotonic());
    }
}

pub(super) fn mark(tag: usize, visible: bool, at: i64) {
    log(|log| {
        let screen = &mut log.screens[tag];
        screen.split = true;
        screen.spans.mark(visible, at);
    });
}

pub(super) fn reopen(at: i64) {
    let shown = counts(|counts| *counts);
    (0..2).filter(|&tag| shown[tag] > 0).for_each(|tag| mark(tag, true, at));
}
