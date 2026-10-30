use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_scene::{Budget, Governor, Heat, Quality, Runtime, SCHEMA, Scene, Tier, Why};

const FRAME: Duration = Duration::from_micros(16_667);
const BUDGET: f32 = 8.3;

fn run(g: &mut Governor, from: Instant, seconds: f32, gpu_ms: f32, raises: bool) -> (Instant, Vec<f32>) {
    let (mut at, mut changes) = (from, Vec::new());
    while at < from + Duration::from_secs_f32(seconds) {
        at += FRAME;
        if g.feed(at, gpu_ms, |_| BUDGET, Quality::Q3, raises) {
            changes.push((at - from).as_secs_f32());
        }
    }
    (at, changes)
}

#[test]
fn long_frames_lower_the_level_step_by_step_and_short_ones_raise_it_back_slowly() {
    let (mut g, start) = (Governor::default(), Instant::now());
    let (at, changes) = run(&mut g, start, 1.5, 12.0, true);
    assert!(changes.is_empty(), "a short burst of slow frames changes nothing");
    let (at, changes) = run(&mut g, at, 8.0, 12.0, true);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert!(changes[0] >= 0.5 && changes[0] < 1.0 && changes[1] - changes[0] >= 5.0, "{changes:?}");
    assert_eq!(g.level((Quality::Q3, Why::Full)), (Quality::Q1, Why::Slow));
    let (at, changes) = run(&mut g, at, 30.0, 6.0, true);
    assert!(changes.is_empty(), "frames between half and the whole budget keep the level");
    let (at, changes) = run(&mut g, at, 14.0, 2.0, true);
    assert!(changes.is_empty(), "raising waits 15 s of fast frames");
    let (_, changes) = run(&mut g, at, 20.0, 2.0, true);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert_eq!(g.level((Quality::Q2, Why::Battery)), (Quality::Q2, Why::Battery));
}

#[test]
fn battery_never_raises_the_level_and_the_floor_holds() {
    let (mut g, start) = (Governor::default(), Instant::now());
    let (at, _) = run(&mut g, start, 60.0, 40.0, false);
    assert_eq!(g.level((Quality::Q3, Why::Full)).0, Quality::Q0);
    assert_eq!(g.level((Quality::Q1, Why::Battery)), (Quality::Q0, Why::Slow));
    let (_, changes) = run(&mut g, at, 60.0, 1.0, false);
    assert!(changes.is_empty(), "on battery the level only goes down");
}

#[test]
fn sporadic_slow_frames_do_not_count_as_a_slow_stretch() {
    let (mut g, mut at) = (Governor::default(), Instant::now());
    for _ in 0..20 {
        at += Duration::from_millis(1500);
        assert!(!g.feed(at, 50.0, |_| BUDGET, Quality::Q3, true));
    }
    assert_eq!(g.level((Quality::Q3, Why::Full)), (Quality::Q3, Why::Full));
}

fn levels(refresh: u32, cost: impl Fn(Quality) -> f32, from: f32, seconds: f32) -> Vec<(f32, Quality)> {
    let mut scene = (*runtime(0).scene).clone();
    scene.clock.max_fps = 0;
    let light = Runtime::new(Arc::new(scene), Default::default());
    let (power, start) = (Budget::new(Tier::T3, false, 0), Instant::now());
    let budget = |q| 1000.0 / power.fps(&light, q, refresh) as f32 * power.gpu_share();
    let (mut g, mut at, mut seen) = (Governor::default(), start, vec![(0.0, Quality::Q3)]);
    while at < start + Duration::from_secs_f32(seconds) {
        let level = g.level((Quality::Q3, Why::Full)).0;
        at += Duration::from_secs(1) / power.fps(&light, level, refresh);
        let t = (at - start).as_secs_f32();
        if g.feed(at, cost(level) * if t < from { 1.0 } else { 0.1 }, budget, Quality::Q3, true) {
            seen.push((t, g.level((Quality::Q3, Why::Full)).0));
        }
    }
    seen
}

#[test]
fn a_lower_level_that_also_lowers_the_frame_rate_does_not_bounce_back() {
    let flat = levels(144, |_| 3.8, f32::MAX, 120.0);
    assert_eq!(flat.iter().map(|s| s.1).collect::<Vec<_>>(), [Quality::Q3, Quality::Q2], "{flat:?}");
    let scaled = |q: Quality| 10.0 * q.render_scale().powi(2);
    let shrunk = levels(60, scaled, f32::MAX, 120.0);
    assert_eq!(shrunk.last().map(|s| s.1), Some(Quality::Q1), "{shrunk:?}");
    assert!(shrunk.windows(2).all(|w| w[1].1 < w[0].1), "the level went back up: {shrunk:?}");
    let lighter = levels(60, scaled, 30.0, 120.0);
    assert_eq!(lighter[..3], shrunk[..3]);
    assert_eq!(
        lighter.last().map(|s| s.1),
        Some(Quality::Q3),
        "a scene that got lighter stayed low: {lighter:?}"
    );
}

fn runtime(layers: usize) -> Runtime {
    let blur = |i: usize| json!({ "id": format!("b{i}"), "type": "adjustment", "effects": [{ "effect": "blur", "params": { "radius": 30 } }] });
    let layers: Vec<_> = (0..layers).map(blur).collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [960, 540] }, "layers": layers });
    Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
}

#[test]
fn the_level_tells_why_it_is_below_the_highest() {
    let (light, heavy) = (runtime(0), runtime(6));
    let mains = |tier| Budget::new(tier, false, 60);
    assert_eq!(mains(Tier::T3).level(&light), (Quality::Q3, Why::Full));
    assert_eq!(mains(Tier::T1).level(&light), (Quality::Q2, Why::Device));
    assert_eq!(Budget::new(Tier::T3, true, 60).level(&light), (Quality::Q2, Why::Battery));
    assert_eq!(Budget { saver: true, ..mains(Tier::T3) }.level(&light), (Quality::Q1, Why::Saver));
    assert_eq!(Budget { heat: Heat::Hot, ..mains(Tier::T3) }.level(&light), (Quality::Q0, Why::Heat));
    assert_eq!(mains(Tier::T2).level(&heavy).1, Why::Cost);
    assert!(mains(Tier::T3).raises() && !Budget::new(Tier::T3, true, 60).raises());
    assert_eq!((mains(Tier::T3).gpu_share(), Budget::new(Tier::T3, true, 60).gpu_share()), (0.5, 0.25));
}
