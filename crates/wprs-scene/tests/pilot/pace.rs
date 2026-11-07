use super::*;
use wprs_scene::{Step, periods};

fn halving(floor: Quality, fitted: Quality) -> Run {
    let mut run = pilot(floor, fitted);
    run.periods = HALVING;
    run
}

fn pairs(run: &Run) -> Vec<(Quality, u32)> {
    run.changes.iter().map(|c| (c.1, c.2)).collect()
}

#[test]
fn a_level_that_fits_only_the_doubled_period_is_climbed_to_at_half_pace() {
    let mut run = halving(Quality::Q0, Quality::Q1);
    run.go(120.0, costs([5.0, 10.0, 22.0, 40.0]));
    assert_eq!(pairs(&run), [(Quality::Q2, 2)], "{:?}", run.changes);
    assert!(run.changes[0].0 >= 6.0, "{:?}", run.changes);
    assert_eq!((run.pilot.pace(), run.pilot.step()), (2, Step::Steady));
    let mut smooth = pilot(Quality::Q0, Quality::Q1);
    smooth.go(120.0, costs([5.0, 10.0, 22.0, 40.0]));
    assert!(smooth.changes.is_empty(), "smooth keeps the level it has: {:?}", smooth.changes);
}

#[test]
fn half_pace_goes_back_to_the_full_pace_before_the_level_climbs_again() {
    let mut run = halving(Quality::Q0, Quality::Q1);
    run.go(60.0, costs([5.0, 10.0, 22.0, 40.0]));
    assert_eq!(pairs(&run), [(Quality::Q2, 2)]);
    run.go(60.0, costs([3.0, 5.0, 7.0, 12.0]));
    assert_eq!(pairs(&run), [(Quality::Q2, 2), (Quality::Q2, 1), (Quality::Q3, 1)], "{:?}", run.changes);
    assert!(run.changes[1].0 - run.changes[0].0 >= 6.0, "{:?}", run.changes);
    assert_eq!((run.pilot.pace(), run.pilot.step()), (1, Step::Top));
}

#[test]
fn an_overload_halves_the_pace_before_it_drops_a_level() {
    let mut run = halving(Quality::Q0, Quality::Q3);
    run.go(40.0, costs([6.0, 10.0, 18.0, 30.0]));
    assert_eq!(pairs(&run), [(Quality::Q3, 2), (Quality::Q2, 2)], "{:?}", run.changes);
    assert!(run.changes[0].0 < 3.0 && run.changes[1].0 - run.changes[0].0 >= 5.0, "{:?}", run.changes);
    let mut low = halving(Quality::Q0, Quality::Q0);
    low.go(30.0, costs([20.0; 4]));
    assert!(low.changes.is_empty(), "the lowest level has no pace to give up: {:?}", low.changes);
}

#[test]
fn smooth_never_changes_the_pace() {
    let mut run = pilot(Quality::Q0, Quality::Q3);
    run.go(30.0, costs([6.0, 10.0, 18.0, 30.0]));
    assert_eq!(pairs(&run), [(Quality::Q2, 1), (Quality::Q1, 1)], "{:?}", run.changes);
    run.go(120.0, costs([2.0, 3.0, 4.0, 5.0]));
    assert!(run.changes.iter().all(|c| c.2 == 1) && run.pilot.pace() == 1, "{:?}", run.changes);
}

#[test]
fn a_cap_or_a_slow_display_leaves_no_half_pace_below_thirty() {
    let ms = |fps: &[u32; 2]| periods(true, *fps).map(|p| (1000.0 / p).round() as u32);
    assert_eq!(
        [ms(&[60, 30]), ms(&[120, 60]), ms(&[30, 15]), ms(&[24, 12]), ms(&[45, 23])],
        [[60, 30], [120, 60], [30, 30], [24, 24], [45, 45]]
    );
    assert_eq!(periods(false, [60, 30]), [1000.0 / 60.0; 2]);
    let mut capped = pilot(Quality::Q0, Quality::Q3);
    capped.periods = periods(true, [30, 15]);
    capped.go(30.0, costs([10.0, 20.0, 40.0, 80.0]));
    assert!(capped.changes.iter().all(|c| c.2 == 1) && !capped.changes.is_empty(), "{:?}", capped.changes);
}

#[test]
fn the_pace_returns_to_full_when_halving_stops_being_offered() {
    let mut run = halving(Quality::Q0, Quality::Q3);
    run.go(10.0, costs([6.0, 10.0, 18.0, 30.0]));
    assert_eq!(run.pilot.pace(), 2);
    run.periods = SMOOTH;
    run.go(1.0, costs([6.0, 10.0, 18.0, 30.0]));
    assert_eq!((run.pilot.pace(), run.pilot.step()), (1, Step::Doubled));
}

#[test]
fn a_pace_that_flips_back_and_forth_waits_longer_each_time() {
    let mut run = halving(Quality::Q0, Quality::Q2);
    run.go_paced(600.0, |_, pace, _| if pace == 1 { 16.0 } else { 6.0 });
    let back: Vec<f32> = run.changes.iter().filter(|c| c.2 == 1).map(|c| c.0).collect();
    assert!(run.changes.len() <= 16, "{:?}", run.changes);
    assert!(back.windows(3).all(|w| w[2] - w[1] >= w[1] - w[0] - 1.0), "{back:?}");
    assert!(run.changes.iter().all(|c| c.1 == Quality::Q2), "{:?}", run.changes);
}

#[test]
fn a_climb_that_does_not_hold_at_full_pace_settles_at_half_pace_without_oscillating() {
    let mut run = halving(Quality::Q0, Quality::Q2);
    run.go(600.0, costs([3.0, 5.0, 8.0, 17.0]));
    assert_eq!(pairs(&run), [(Quality::Q3, 1), (Quality::Q3, 2)], "{:?}", run.changes);
    assert_eq!((run.pilot.pace(), run.pilot.step()), (2, Step::Top));
}
