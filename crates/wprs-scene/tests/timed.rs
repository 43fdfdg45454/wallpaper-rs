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
