use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};
use wprs_scene::Tier;

const BUDGETS: [f64; 4] = [400.0, 60.0, 40.0, 30.0];
const OVER: usize = 3;
const AT_ONCE: f64 = 100.0;
const WATCHED: f64 = 1000.0;
const WINDOW: Duration = Duration::from_secs(8);
const HELD: usize = 256;
const SLOW: &str = "it is too slow for this graphics card";
static BUDGET_US: AtomicU64 = AtomicU64::new(0);

pub fn set_gpu_budget(limit: Option<Duration>) {
    let us = limit.map_or(0, |l| u64::try_from(l.as_micros()).unwrap_or(u64::MAX).max(1));
    BUDGET_US.store(us, Relaxed);
}

pub(super) struct Judge {
    budget: f64,
    over: VecDeque<(Instant, u64)>,
    units: Vec<(u64, f64)>,
}

impl Judge {
    pub(super) fn new(tier: Tier) -> Judge {
        Judge { budget: BUDGETS[tier as usize], over: VecDeque::new(), units: Vec::new() }
    }

    pub(super) fn suspect(&self) -> bool {
        !self.over.is_empty()
    }

    fn budget(&self) -> f64 {
        match BUDGET_US.load(Relaxed) {
            0 => self.budget,
            us => us as f64 / 1000.0,
        }
    }

    pub(super) fn watched(&mut self, keys: &[u64], ms: f64) -> Vec<(u64, String)> {
        let budget = self.budget();
        let late = ms > (budget * 4.0).max(WATCHED);
        keys.iter().filter(|_| late).map(|k| (*k, why(ms, ms, budget))).collect()
    }

    pub(super) fn frame(
        &mut self,
        runs: &[(u64, f64)],
        whole: Option<f64>,
        now: Instant,
    ) -> Vec<(u64, String)> {
        let budget = self.budget();
        self.units.clear();
        for (key, ms) in runs {
            match self.units.iter_mut().find(|u| u.0 == *key) {
                Some(unit) => unit.1 += ms,
                None => self.units.push((*key, *ms)),
            }
        }
        let timed: f64 = self.units.iter().map(|u| u.1).sum();
        let total = whole.filter(|_| timed > 0.0).map_or(timed, |w| w.max(timed));
        self.units.iter_mut().for_each(|u| u.1 *= total / timed.max(f64::MIN_POSITIVE));
        self.units.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.over.retain(|(at, _)| now.saturating_duration_since(*at) < WINDOW);
        let (mut rest, mut charged) = (total, Vec::new());
        for &(key, ms) in &self.units {
            if rest <= budget {
                break;
            }
            rest -= ms;
            self.over.push_back((now, key));
            let count = self.over.iter().filter(|o| o.1 == key).count();
            if ms > (budget * 4.0).max(AT_ONCE) || count >= OVER {
                charged.push((key, why(total, ms, budget)));
            }
        }
        let old = self.over.len().saturating_sub(HELD);
        self.over.drain(..old);
        charged
    }
}

fn why(total: f64, ms: f64, budget: f64) -> String {
    format!(
        "{SLOW}: a frame took {total:.0} ms on it, {ms:.0} ms of them in this effect, and it allows {budget:.0} ms"
    )
}
