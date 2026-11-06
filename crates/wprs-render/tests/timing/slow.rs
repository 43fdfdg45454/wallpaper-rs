use super::hostile::{entry, renderer, scene, show};
use super::{book, busy, draw, lines, one, wide};
use std::time::Duration;
use wprs_render::{Renderer, drawn_effects, retry_effects, set_gpu_budget};

fn effects() -> wprs_scene::Scene {
    scene(&[("busy", busy(BUSY)), ("fine", entry("return tap(in.uv) * 0.5;"))])
}

fn judged(r: &Renderer, effect: String, size: (u32, u32)) -> f64 {
    retry_effects();
    set_gpu_budget(Some(Duration::from_micros(1)));
    let mut shown = show(r, scene(&[("probe", effect)])).unwrap();
    for _ in 0..40 {
        draw(r, &mut shown, size);
        if let Some(said) = shown.view.left_out().first() {
            let before = said.split(" ms of them in this effect").next().unwrap_or_default();
            return before.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
        }
    }
    0.0
}

fn calibrated(r: &Renderer, size: (u32, u32)) -> Duration {
    let cheap = judged(r, entry("return tap(in.uv) * 0.5;"), size).max(0.5);
    let heavy = judged(r, busy(BUSY), size);
    set_gpu_budget(None);
    retry_effects();
    assert!(
        heavy > cheap * 4.0,
        "the busy effect is not much slower than the cheap one: {heavy} ms, {cheap} ms"
    );
    Duration::from_secs_f64((cheap * heavy).sqrt() / 1000.0)
}

#[test]
fn slow_effects_are_left_out_for_the_run_without_a_strike_and_never_queue_up() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let size = wide(r);
    let budget = calibrated(r, size);
    retry_effects();
    let path = book("busy");
    let s = effects();
    let key = s.loaded["busy"].key;
    set_gpu_budget(Some(budget));
    let mut shown = show(r, s).unwrap();
    for _ in 0..40 {
        draw(r, &mut shown, size);
        let flying = shown.view.stats().in_flight;
        assert!(flying <= 2, "{flying} frames of one view were on the GPU at once");
        if !shown.view.left_out().is_empty() {
            break;
        }
    }
    let left = shown.view.left_out();
    let allows = format!("allows {:.0} ms", budget.as_secs_f64() * 1000.0);
    assert!(
        left.len() == 1 && left[0].contains("busy") && left[0].contains("too slow for this graphics card"),
        "the cheap effect stays within {budget:?}: {left:?}"
    );
    assert!(
        !left[0].contains("stopped") && left[0].contains("ms on it") && left[0].contains(&allows),
        "{left:?}"
    );
    assert_eq!(lines(&path, key, "1"), 0, "{}", std::fs::read_to_string(&path).unwrap_or_default());
    r.finish().unwrap();
    let frames: Vec<_> = (0..3)
        .map(|_| {
            draw(r, &mut shown, size);
            let stats = shown.view.stats();
            (stats.compiled, stats.created)
        })
        .collect();
    r.finish().unwrap();
    assert!(
        frames.iter().all(|f| f.0 == 0 && f.1 <= 1),
        "frames without the busy effect built pipelines or objects: {frames:?}"
    );
    assert!(!drawn_effects().contains(&key));
    set_gpu_budget(None);
    assert!(retry_effects() >= 1 && shown.view.retry(), "a view with an effect left out is built again");
    shown = show(r, effects()).unwrap();
    draw(r, &mut shown, (64, 32));
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    r.finish().unwrap();
}

const BUSY: usize = 8;
