use std::time::{Duration, Instant};
use wprs_scene::{Budget, Heat, Pilot, Quality, Step, Tier, Why, Window};

const FRAME: Duration = Duration::from_micros(16_667);
const PERIOD: f32 = 16.667;

struct Run {
    pilot: Pilot,
    at: Instant,
    start: Instant,
    ceiling: Quality,
    stills: bool,
    changes: Vec<(f32, Quality)>,
}

fn pilot(floor: Quality, fitted: Quality) -> Run {
    let mut pilot = Pilot::default();
    pilot.start(floor, fitted);
    let at = Instant::now();
    Run { pilot, at, start: at, ceiling: Quality::Q3, stills: true, changes: Vec::new() }
}

impl Run {
    fn level(&self) -> Quality {
        self.pilot.level((self.ceiling, Why::Full), Why::Cost).0
    }

    fn go(&mut self, seconds: f32, ms: impl Fn(Quality, u32) -> f32) -> &mut Run {
        let end = self.at + Duration::from_secs_f32(seconds);
        let mut k = 0;
        while self.at < end && !self.pilot.stalled() {
            (self.at, k) = (self.at + FRAME, k + 1);
            let here = self.level();
            if self.pilot.feed(self.at, ms(here, k), PERIOD, (self.ceiling, true, self.stills)) {
                self.changes.push(((self.at - self.start).as_secs_f32(), self.level()));
            }
        }
        self
    }
}

fn costs(table: [f32; 4]) -> impl Fn(Quality, u32) -> f32 {
    move |q, _| table[q as usize]
}

#[test]
fn the_window_waits_for_enough_frames_and_leaves_out_lone_spikes() {
    let mut w = Window::default();
    (0..11).for_each(|_| w.push(8.0));
    assert_eq!(w.mean(), None, "eleven frames are not enough to judge");
    w.push(8.0);
    assert_eq!(w.mean(), Some(8.0));
    (0..3).for_each(|_| w.push(200.0));
    assert_eq!(w.mean(), Some(8.0), "three spikes in a window of 32 do not move it");
    (0..32).for_each(|k| w.push(if k % 2 == 0 { 9.0 } else { 11.0 }));
    assert_eq!(w.mean(), Some(10.0), "noise averages out");
    (0..20).for_each(|_| w.push(30.0));
    assert_eq!(w.mean(), Some(30.0), "a sustained change takes over the window");
}

#[test]
fn a_level_with_frames_between_the_two_lines_stays() {
    let mut run = pilot(Quality::Q0, Quality::Q1);
    run.go(120.0, costs([5.0, 11.0, 20.0, 30.0]));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!((run.level(), run.pilot.step()), (Quality::Q1, Step::Steady));
    assert_eq!(run.pilot.level((Quality::Q3, Why::Full), Why::Cost), (Quality::Q1, Why::Slow));
    assert!((run.pilot.frame_ms().unwrap() - 11.0).abs() < 1e-3);
}

#[test]
fn frames_with_room_to_spare_climb_from_the_model_level_to_the_top_one_step_at_a_time() {
    let mut run = pilot(Quality::Q0, Quality::Q0);
    assert_eq!(run.pilot.level((Quality::Q3, Why::Full), Why::Cost), (Quality::Q0, Why::Cost));
    assert_eq!(run.pilot.step(), Step::Model);
    run.go(60.0, costs([3.0, 5.0, 7.0, 12.0]));
    let levels: Vec<_> = run.changes.iter().map(|c| c.1).collect();
    assert_eq!(levels, [Quality::Q1, Quality::Q2, Quality::Q3], "{:?}", run.changes);
    assert!(run.changes[0].0 >= 6.0 && run.changes[2].0 < 30.0, "{:?}", run.changes);
    assert!(run.changes.windows(2).all(|w| w[1].0 - w[0].0 >= 5.0), "{:?}", run.changes);
    assert_eq!((run.level(), run.pilot.step()), (Quality::Q3, Step::Top));
    assert_eq!(run.pilot.level((Quality::Q3, Why::Full), Why::Cost), (Quality::Q3, Why::Full));
}

#[test]
fn a_sustained_overload_drops_a_level_within_seconds_and_keeps_dropping_while_it_lasts() {
    let mut run = pilot(Quality::Q0, Quality::Q3);
    run.go(30.0, costs([6.0, 10.0, 18.0, 30.0]));
    let levels: Vec<_> = run.changes.iter().map(|c| c.1).collect();
    assert_eq!(levels, [Quality::Q2, Quality::Q1], "{:?}", run.changes);
    assert!(run.changes[0].0 < 3.0 && run.changes[1].0 - run.changes[0].0 < 8.0, "{:?}", run.changes);
    assert_eq!(run.pilot.level((Quality::Q3, Why::Full), Why::Cost), (Quality::Q1, Why::Slow));
}

#[test]
fn a_spike_of_half_a_second_does_not_drop_the_level() {
    let mut run = pilot(Quality::Q0, Quality::Q3);
    let spiky = |_: Quality, k: u32| if (600..630).contains(&(k % 1200)) { 120.0 } else { 9.0 };
    run.go(120.0, spiky);
    assert!(run.changes.is_empty(), "{:?}", run.changes);
}

#[test]
fn noisy_frames_around_a_fitting_mean_never_change_the_level() {
    let mut run = pilot(Quality::Q0, Quality::Q2);
    let noisy = |q: Quality, k: u32| {
        [4.0, 6.0, 10.0, 18.0][q as usize] * (0.6 + 0.8 * ((k * 7919) % 101) as f32 / 100.0)
    };
    run.go(300.0, noisy);
    assert!(run.changes.is_empty(), "{:?}", run.changes);
}

#[test]
fn a_level_above_that_did_not_fit_is_left_alone_so_the_level_never_oscillates() {
    let mut run = pilot(Quality::Q0, Quality::Q2);
    run.go(600.0, costs([3.0, 5.0, 7.5, 17.0]));
    let levels: Vec<_> = run.changes.iter().map(|c| c.1).collect();
    assert_eq!(levels, [Quality::Q3, Quality::Q2], "{:?}", run.changes);
    assert_eq!((run.level(), run.pilot.step()), (Quality::Q2, Step::Held));
    let changes = run.changes.len();
    run.go(30.0, costs([2.0, 3.0, 4.0, 9.0]));
    assert_eq!(run.changes.len(), changes + 1, "a lighter stretch climbs again: {:?}", run.changes);
    assert_eq!(run.level(), Quality::Q3);
}

#[test]
fn only_automatic_quality_goes_still_and_only_when_even_the_lowest_level_is_far_too_slow() {
    let mut run = pilot(Quality::Q0, Quality::Q0);
    run.go(60.0, costs([25.0; 4]));
    assert!(!run.pilot.stalled(), "twice the period is still worth playing");
    run.go(10.0, costs([40.0; 4]));
    assert!(run.pilot.stalled() && run.pilot.step() == Step::Still);
    run.pilot.start(Quality::Q0, Quality::Q1);
    assert!(run.pilot.stalled(), "a new start of the same choice keeps it still");
    run.pilot.start(Quality::Q1, Quality::Q1);
    assert!(!run.pilot.stalled(), "another choice tries again");
    let mut floor = pilot(Quality::Q1, Quality::Q1);
    floor.stills = false;
    floor.go(30.0, costs([40.0; 4]));
    assert!(!floor.pilot.stalled() && floor.changes.is_empty() && floor.level() == Quality::Q1);
}

#[test]
fn a_level_the_user_fixed_is_never_touched() {
    let mut run = pilot(Quality::Q1, Quality::Q1);
    run.ceiling = Quality::Q1;
    run.stills = false;
    run.go(60.0, costs([1.0; 4]));
    run.go(60.0, costs([90.0; 4]));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.pilot.level((Quality::Q1, Why::User), Why::Cost), (Quality::Q1, Why::User));
}

#[test]
fn the_ceiling_ignores_the_battery_and_the_tier_but_not_the_saver_or_the_heat() {
    let battery = Budget::new(Tier::T1, true, 60);
    assert_eq!(battery.reach(), (Quality::Q3, Why::Full));
    assert_eq!(Budget { saver: true, ..battery }.reach(), (Quality::Q0, Why::Saver));
    assert_eq!(Budget { heat: Heat::Warm, ..battery }.reach(), (Quality::Q2, Why::Heat));
}
