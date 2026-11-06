use std::time::{Duration, Instant};
use wprs_scene::{Budget, Governor, Heat, Quality, Tier, Why};

const FRAME: Duration = Duration::from_micros(16_667);
const LIMIT: f32 = 12.5;
const COSTS: [f32; 4] = [2.0, 5.0, 7.0, 14.0];

fn run(g: &mut Governor, from: Instant, seconds: f32, gpu_ms: impl Fn(Quality) -> f32) -> Instant {
    let mut at = from;
    while at < from + Duration::from_secs_f32(seconds) {
        at += FRAME;
        let here = g.level((Quality::Q3, Why::Full)).0;
        g.feed(at, gpu_ms(here), |q| (LIMIT, COSTS[q as usize]), Quality::Q3, true);
    }
    at
}

#[test]
fn a_timed_governor_starts_at_the_fitted_level_and_climbs_while_the_level_above_fits() {
    let mut g = Governor::timed();
    g.begin(Quality::Q3, Quality::Q0);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q0);
    let phone = |q: Quality| [3.0, 7.0, 9.5, 19.0][q as usize];
    let at = run(&mut g, Instant::now(), 70.0, phone);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q2);
    let at = run(&mut g, at, 120.0, phone);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q2, "Q3 needs 19 ms of a 12.5 ms budget");
    let faster = |q: Quality| [2.0, 4.0, 5.0, 10.0][q as usize];
    run(&mut g, at, 60.0, faster);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q3);
}

#[test]
fn the_timed_ceiling_ignores_the_battery_and_the_tier_but_not_the_saver_or_the_heat() {
    let battery = Budget::new(Tier::T1, true, 60);
    assert_eq!(battery.reach(), (Quality::Q3, Why::Full));
    assert_eq!(Budget { saver: true, ..battery }.reach(), (Quality::Q0, Why::Saver));
    assert_eq!(Budget { heat: Heat::Warm, ..battery }.reach(), (Quality::Q2, Why::Heat));
}

fn raises(g: &mut Governor, from: Instant, seconds: f32, gpu_ms: impl Fn(Quality) -> f32) -> Vec<f32> {
    let (mut at, mut seen) = (from, Vec::new());
    while at < from + Duration::from_secs_f32(seconds) {
        at += FRAME;
        let here = g.level((Quality::Q3, Why::Full)).0;
        g.feed(at, gpu_ms(here), |q| (LIMIT, COSTS[q as usize]), Quality::Q3, true);
        if g.level((Quality::Q3, Why::Full)).0 > here {
            seen.push((at - from).as_secs_f32());
        }
    }
    seen
}

#[test]
fn a_timed_governor_tries_the_level_above_when_the_model_overrates_it_and_keeps_it_when_it_fits() {
    let mut g = Governor::timed();
    g.begin(Quality::Q3, Quality::Q2);
    let phone = |q: Quality| [2.0, 5.0, 8.0, 11.5][q as usize];
    let seen = raises(&mut g, Instant::now(), 120.0, phone);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!(seen[0] >= 15.0 && seen[0] < 17.0, "{seen:?}");
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q3);
}

#[test]
fn a_failed_try_steps_back_and_the_next_tries_wait_twice_as_long_each_time() {
    let mut g = Governor::timed();
    g.begin(Quality::Q3, Quality::Q2);
    let phone = |q: Quality| [3.0, 7.0, 9.5, 19.0][q as usize];
    let seen = raises(&mut g, Instant::now(), 900.0, phone);
    let gaps: Vec<f32> = seen.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(seen.len() >= 4 && seen[0] < 17.0, "{seen:?}");
    assert!(gaps.windows(2).all(|w| w[1] >= w[0] * 1.5 || w[1] > 240.0), "{gaps:?}");
    assert!(gaps.iter().all(|g| *g < 260.0), "the tries stop at four minutes apart: {gaps:?}");
}

#[test]
fn a_measured_try_replaces_the_model_so_a_lighter_scene_climbs_without_waiting_for_the_next_try() {
    let mut g = Governor::timed();
    g.begin(Quality::Q3, Quality::Q2);
    let heavy = |q: Quality| [3.0, 7.0, 9.5, 14.0][q as usize];
    let seen = raises(&mut g, Instant::now(), 100.0, heavy);
    assert!(seen.len() >= 2, "{seen:?}");
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q2);
    let light = |q: Quality| [2.0, 5.0, 7.0, 10.3][q as usize];
    let seen = raises(&mut g, Instant::now() + Duration::from_secs(100), 20.0, light);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q3);
}
