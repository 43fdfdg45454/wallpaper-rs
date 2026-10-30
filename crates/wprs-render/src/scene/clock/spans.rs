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
fn package_passes_of_one_effect_in_a_row_share_one_timed_run() {
    let (a, b) = (fill(Shader::Own(7, 0, 0)), fill(Shader::Own(9, 0, 0)));
    let ops =
        [fill(Shader::Fx(1, "fs_copy")), a, fill(Shader::Own(7, 1, 0)), b, Op::Clear(Src::Canvas, [0.0; 4])];
    let mut p = Plan::new();
    p.plan(&ops, 0);
    assert!(p.runs == [7, 9] && !p.whole);
    assert_eq!(p.marks, [(None, None), (Some(0), None), (None, Some(1)), (Some(2), Some(3)), (None, None)]);
}

#[test]
fn a_frame_with_more_spans_than_slots_always_times_its_first_and_last_span() {
    let n = SLOTS as u64 + 5;
    let many: Vec<Op> =
        (0..n).flat_map(|k| [fill(Shader::Own(k, 0, 0)), Op::Clear(Src::Canvas, [0.0; 4])]).collect();
    let (mut p, mut seen) = (Plan::new(), std::collections::HashSet::new());
    for turn in 0..3 {
        p.plan(&many, turn);
        assert!(p.whole && p.runs.len() == SLOTS && p.runs.windows(2).all(|w| w[0] < w[1]));
        assert!(p.runs[0] == 0 && p.runs[SLOTS - 1] == n - 1);
        assert!(p.marks.iter().all(|m| m.0.map(|b| b / 2) == m.1.map(|e| e / 2)));
        seen.extend(p.runs.iter().copied());
    }
    assert_eq!(seen.len(), SLOTS + 5, "every effect is timed within a few frames");
}

#[test]
fn the_whole_frame_is_judged_and_shared_by_the_timed_effects() {
    let (mut judge, start) = (Judge::new(Tier::T3), Instant::now());
    let runs = [(1, 3.0), (2, 1.0)];
    let charged = judge.frame(&runs, Some(280.0), start);
    assert!(charged.len() == 1 && charged[0].0 == 1, "{charged:?}");
    assert!(charged[0].1.contains("280 ms on it, 210 ms"), "{charged:?}");
    let mut judge = Judge::new(Tier::T3);
    let fine = (0..6u64).flat_map(|k| judge.frame(&runs, Some(20.0), start + Duration::from_millis(k * 16)));
    assert_eq!(fine.count(), 0);
}
