use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_scene::{Budget, Governor, Quality, Runtime, SCHEMA, Scene, Tier, Why};

type Seen = Vec<(f32, Quality)>;

fn levels(
    refresh: u32,
    model: impl Fn(Quality) -> f32,
    cost: impl Fn(Quality, f32) -> f32,
    seconds: f32,
) -> Seen {
    let text = format!(r#"{{ "schema": "{SCHEMA}", "canvas": {{ "size": [960, 540] }}, "layers": [] }}"#);
    let mut scene = Scene::from_json(&text).unwrap();
    scene.clock.max_fps = 0;
    let light = Runtime::new(Arc::new(scene), Default::default());
    let (power, start) = (Budget::new(Tier::T3, false, 0), Instant::now());
    let budget = |q| (1000.0 / power.fps(&light, q, refresh) as f32 * power.gpu_share(), model(q));
    let (mut g, mut at, mut seen) = (Governor::default(), start, vec![(0.0, Quality::Q3)]);
    while at < start + Duration::from_secs_f32(seconds) {
        let level = g.level((Quality::Q3, Why::Full)).0;
        at += Duration::from_secs(1) / power.fps(&light, level, refresh);
        let t = (at - start).as_secs_f32();
        if g.feed(at, cost(level, t), budget, Quality::Q3, true) {
            seen.push((t, g.level((Quality::Q3, Why::Full)).0));
        }
    }
    seen
}

fn path(seen: &Seen) -> Vec<Quality> {
    seen.iter().map(|s| s.1).collect()
}

fn scaled(q: Quality) -> f32 {
    10.0 * q.render_scale().powi(2)
}

#[test]
fn a_lower_level_that_also_lowers_the_frame_rate_does_not_bounce_back() {
    let flat = levels(144, |_| 1.0, |_, _| 3.8, 120.0);
    assert_eq!(path(&flat), [Quality::Q3, Quality::Q2], "{flat:?}");
    let shrunk = levels(60, scaled, |q, _| scaled(q), 120.0);
    assert_eq!(shrunk.last().map(|s| s.1), Some(Quality::Q1), "{shrunk:?}");
    assert!(shrunk.windows(2).all(|w| w[1].1 < w[0].1), "the level went back up: {shrunk:?}");
    let lighter = levels(60, scaled, |q, t| scaled(q) * if t < 30.0 { 1.0 } else { 0.1 }, 120.0);
    assert_eq!(lighter[..3], shrunk[..3]);
    assert_eq!(
        lighter.last().map(|s| s.1),
        Some(Quality::Q3),
        "a scene that got lighter stayed low: {lighter:?}"
    );
}

#[test]
fn a_burst_that_ends_right_after_a_step_down_does_not_keep_the_level_low() {
    let (q3, q2, q1) = (Quality::Q3, Quality::Q2, Quality::Q1);
    for (refresh, peak, rest, burst, want) in [
        (60, 12.0, 2.0, 4.0, vec![q3, q2, q3]),
        (60, 12.0, 1.0, 4.0, vec![q3, q2, q3]),
        (60, 12.0, 2.0, 8.0, vec![q3, q2, q1, q2, q3]),
        (144, 6.0, 1.0, 4.0, vec![q3, q2, q3]),
    ] {
        let seen = levels(refresh, |_| 1.0, |_, t| if t < burst { peak } else { rest }, 600.0);
        assert_eq!(path(&seen), want, "{refresh} Hz, {peak} ms for {burst} s: {seen:?}");
        assert!(seen.last().unwrap().0 < 60.0, "{seen:?}");
    }
}

#[test]
fn a_gain_kept_from_a_burst_is_tried_again_after_a_minute() {
    let model = |q| if q == Quality::Q3 { 8.0 } else { 1.0 };
    let seen = levels(60, model, |_, t| if t < 4.0 { 12.0 } else { 2.0 }, 600.0);
    assert_eq!(path(&seen), [Quality::Q3, Quality::Q2, Quality::Q3], "{seen:?}");
    assert!((60.0..80.0).contains(&seen[2].0), "{seen:?}");
}

#[test]
fn a_level_above_that_stays_too_dear_is_tried_less_and_less_often() {
    let cost = |q: Quality| if q == Quality::Q3 { 9.0 } else { 2.5 };
    for known in [false, true] {
        let seen = levels(60, |q| if known { cost(q) } else { 1.0 }, |q, _| cost(q), 1200.0);
        let raises: Vec<f32> = seen.windows(2).filter(|w| w[1].1 > w[0].1).map(|w| w[1].0).collect();
        let gaps: Vec<f32> = raises.windows(2).map(|w| w[1] - w[0]).collect();
        assert!((3..=8).contains(&raises.len()), "{seen:?}");
        assert!(gaps.windows(2).all(|w| w[1] >= w[0] - 1.0), "{gaps:?}");
        assert!(gaps.last().is_some_and(|g| *g >= 120.0), "{gaps:?}");
    }
}
