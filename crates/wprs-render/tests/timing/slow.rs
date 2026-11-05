use super::hostile::{Shown, entry, renderer, scene, show};
use super::{book, busy, draw, lines, one, wide};
use std::time::Duration;
use wprs_render::{Renderer, drawn_effects, retry_effects, set_gpu_budget};

fn fine() -> (&'static str, String) {
    ("fine", entry("return tap(in.uv) * 0.5;"))
}

fn effects() -> wprs_scene::Scene {
    scene(&[("busy", busy(BUSY)), fine()])
}

fn cost(r: &Renderer, s: wprs_scene::Scene, size: (u32, u32)) -> f64 {
    let mut shown = show(r, s).unwrap();
    let most = (0..4).fold(0, |most, _| {
        draw(r, &mut shown, size);
        r.finish().unwrap();
        most.max(shown.view.stats().gpu_us)
    });
    f64::from(most.max(1)) / 1000.0
}

fn budget(r: &Renderer, size: (u32, u32)) -> u64 {
    set_gpu_budget(Some(Duration::from_secs(60)));
    let (light, whole) = (cost(r, scene(&[fine()]), size), cost(r, effects(), size));
    ((whole - light) / 4.0).round().max(2.0) as u64
}

fn attempt(r: &Renderer) -> (Shown, Result<(), String>) {
    let size = wide(r);
    let ms = budget(r, size);
    retry_effects();
    let path = book("busy");
    let s = effects();
    let key = s.loaded["busy"].key;
    set_gpu_budget(Some(Duration::from_millis(ms)));
    let mut shown = show(r, s).unwrap();
    let mut slowest = Duration::ZERO;
    for _ in 0..40 {
        slowest = slowest.max(draw(r, &mut shown, size));
        let flying = shown.view.stats().in_flight;
        assert!(flying <= 2, "{flying} frames of one view were on the GPU at once");
        if !shown.view.left_out().is_empty() {
            break;
        }
    }
    let left = shown.view.left_out();
    if left.is_empty() || left.iter().any(|l| l.contains("effect fine")) {
        r.finish().unwrap();
        return (shown, Err(format!("the load changed against a budget of {ms} ms: {left:?}")));
    }
    assert!(
        left.len() == 1 && left[0].contains("busy") && left[0].contains("too slow for this graphics card"),
        "{left:?}"
    );
    assert!(
        !left[0].contains("stopped")
            && left[0].contains("ms on it")
            && left[0].contains(&format!("allows {ms} ms")),
        "{left:?}"
    );
    assert_eq!(lines(&path, key, "1"), 0, "{}", std::fs::read_to_string(&path).unwrap_or_default());
    r.finish().unwrap();
    let frames: Vec<_> = (0..3)
        .map(|_| {
            let took = draw(r, &mut shown, size);
            let stats = shown.view.stats();
            (took, stats.compiled, stats.created)
        })
        .collect();
    r.finish().unwrap();
    assert!(
        frames.iter().all(|f| f.1 == 0 && f.2 <= 1),
        "frames without the busy effect built pipelines or objects: {frames:?}"
    );
    assert!(!drawn_effects().contains(&key));
    let fast = frames.iter().map(|f| f.0).max().unwrap_or_default();
    let miss = format!("frames without the busy effect took {fast:?}, with it {slowest:?}");
    (shown, (fast * 2 < slowest).then_some(()).ok_or(miss))
}

#[test]
fn slow_effects_are_left_out_for_the_run_without_a_strike_and_never_queue_up() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let mut misses = Vec::new();
    let mut shown = loop {
        match attempt(r) {
            (shown, Ok(())) => break shown,
            (_, Err(miss)) => misses.push(miss),
        }
        assert!(misses.len() < 3, "{misses:?}");
    };
    set_gpu_budget(None);
    assert!(retry_effects() >= 1 && shown.view.retry(), "a view with an effect left out is built again");
    shown = show(r, effects()).unwrap();
    draw(r, &mut shown, (64, 32));
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    r.finish().unwrap();
}

const BUSY: usize = 8;
