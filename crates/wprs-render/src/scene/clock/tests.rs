use super::judge::Judge;
use super::{Clock, Timing};
use crate::Renderer;
use std::time::{Duration, Instant};
use wprs_scene::Tier;

fn keys(charged: Vec<(u64, String)>) -> Vec<u64> {
    let mut keys: Vec<u64> = charged.into_iter().map(|c| c.0).collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[test]
fn only_the_effects_that_keep_frames_over_budget_are_charged_within_a_window() {
    let (mut judge, start) = (Judge::new(Tier::T3), Instant::now());
    let at = |ms: u64| start + Duration::from_millis(ms);
    let frame = [(1, 30.0), (2, 5.0), (3, 0.5), (4, 0.5)];
    assert!(judge.frame(&frame, None, at(0)).is_empty() && judge.frame(&frame, None, at(16)).is_empty());
    let charged = judge.frame(&frame, None, at(32));
    assert_eq!(keys(charged.clone()), [1]);
    assert!(
        charged[0].1.contains("36 ms on it, 30 ms") && charged[0].1.contains("allows 30 ms"),
        "{charged:?}"
    );
    let both = [(5, 25.0), (5, 25.0), (6, 35.0)];
    judge.frame(&both, None, at(48));
    judge.frame(&both, None, at(64));
    assert_eq!(keys(judge.frame(&both, None, at(80))), [5, 6]);
    assert_eq!(judge.frame(&[(7, 150.0)], None, at(96)).len(), 1, "a frame over 4 budgets counts at once");
}

#[test]
fn cheap_frames_in_between_do_not_clear_the_count() {
    let (mut judge, start) = (Judge::new(Tier::T3), Instant::now());
    let mut charged = Vec::new();
    for k in 0..12u64 {
        let ms = if k % 2 == 0 { 90.0 } else { 0.0 };
        charged.extend(judge.frame(&[(9, ms)], None, start + Duration::from_millis(k * 300)));
    }
    assert_eq!(keys(charged), [9], "slow frames between cheap ones add up");
    let mut judge = Judge::new(Tier::T3);
    let spaced = (0..6u64).flat_map(|k| judge.frame(&[(9, 90.0)], None, start + Duration::from_secs(k * 5)));
    assert!(spaced.count() == 0, "a few slow frames far apart are forgiven");
}

#[test]
fn frames_timed_on_the_cpu_only_charge_past_a_second() {
    let mut judge = Judge::new(Tier::T3);
    for _ in 0..10 {
        assert!(judge.watched(&[1, 2], 330.0).is_empty());
    }
    assert_eq!(judge.watched(&[1, 2], 1500.0).len(), 2);
}

impl Clock {
    pub(crate) fn again(&mut self, r: &Renderer, ops: &[super::super::Op]) -> Timing {
        drop(self.collect());
        self.start(r, ops, false, true)
    }
}
