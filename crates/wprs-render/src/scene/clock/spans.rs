use super::super::pipes::{Mode, Shader};
use super::super::{Fill, Op, Src};
use super::judge::Judge;
use super::plan::{Plan, SLOTS};
use std::time::{Duration, Instant};
use wprs_scene::Tier;

fn fill(shader: Shader) -> Op {
    let (target, mode) = (Src::Canvas, Mode::Replace);
    Op::Effect(Fill { target, shader, slot: 0, tex: 0, more: None, mode, load: false, vertices: 3 })
}

#[test]
fn passes_of_one_effect_in_a_row_share_one_timed_run_after_the_whole_frame() {
    let (a, b) = (fill(Shader::Own(7, 0, 0)), fill(Shader::Own(9, 0, 0)));
    let ops =
        [fill(Shader::Fx(1, "fs_copy")), a, fill(Shader::Own(7, 1, 0)), b, Op::Clear(Src::Canvas, [0.0; 4])];
    let mut p = Plan::new();
    p.plan(&ops, 0);
    assert!(p.runs == [(1, false), (7, true), (9, true)] && !p.wide);
    let marks = [(Some(2), Some(3)), (Some(4), None), (None, Some(5)), (Some(6), Some(7)), (None, None)];
    assert_eq!(p.marks, marks);
}

#[test]
fn a_frame_with_more_spans_than_slots_times_each_of_them_within_a_few_frames() {
    let n = SLOTS as u64 + 5;
    let many: Vec<Op> =
        (0..n).flat_map(|k| [fill(Shader::Own(k, 0, 0)), Op::Clear(Src::Canvas, [0.0; 4])]).collect();
    let (mut p, mut seen) = (Plan::new(), std::collections::HashSet::new());
    for turn in 0..3 {
        p.plan(&many, turn);
        assert!(p.wide && p.runs.len() == SLOTS - 1 && p.runs.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(p.marks.iter().all(|m| m.0.map(|b| b / 2) == m.1.map(|e| e / 2) && m.0 != Some(0)));
        seen.extend(p.runs.iter().map(|r| r.0));
    }
    assert_eq!(seen.len(), SLOTS + 5, "every effect is timed within a few frames");
}

#[test]
fn the_whole_frame_is_shared_by_the_timed_effects_when_some_went_untimed() {
    let (mut judge, start) = (Judge::new(Tier::T3), Instant::now());
    let runs = [(1, true, 3.0), (2, true, 1.0)];
    let charged = judge.frame(&runs, (Some(280.0), true), start);
    assert!(charged.len() == 1 && charged[0].0 == 1, "{charged:?}");
    assert!(charged[0].1.contains("280 ms on it, 210 ms"), "{charged:?}");
    let mut judge = Judge::new(Tier::T3);
    let at = |k: u64| start + Duration::from_millis(k * 16);
    let fine = (0..6u64).flat_map(|k| judge.frame(&runs, (Some(20.0), true), at(k)));
    assert_eq!(fine.count(), 0);
}

#[test]
fn package_effects_are_charged_for_their_own_time_never_for_what_the_rest_of_the_frame_costs() {
    let (mut judge, start) = (Judge::new(Tier::T3), Instant::now());
    let at = |k: u64| start + Duration::from_millis(k * 16);
    let slow = [(1, false, 50.0), (2, true, 5.0)];
    let none = (0..6u64).flat_map(|k| judge.frame(&slow, (Some(60.0), false), at(k)));
    assert_eq!(none.count(), 0, "a frame over budget without its package effects charges none of them");
    let light = [(1, false, 25.0), (2, true, 10.0)];
    let none = (6..12u64).flat_map(|k| judge.frame(&light, (Some(36.0), false), at(k)));
    assert_eq!(none.count(), 0, "a package effect within the budget was charged for the built-in ones");
    let heavy = [(1, false, 20.0), (2, true, 35.0)];
    let charged: Vec<_> = (12..15u64).flat_map(|k| judge.frame(&heavy, (Some(56.0), false), at(k))).collect();
    assert!(charged.len() == 1 && charged[0].0 == 2, "{charged:?}");
    assert!(charged[0].1.contains("56 ms on it, 35 ms"), "{charged:?}");
    let (mut judge, hung) = (Judge::new(Tier::T0), [(1, false, 450.0), (2, true, 3000.0)]);
    let charged = judge.frame(&hung, (Some(3460.0), false), start);
    assert!(
        charged.len() == 1 && charged[0].0 == 2,
        "a very slow effect waited for more frames: {charged:?}"
    );
}
