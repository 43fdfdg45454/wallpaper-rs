use super::hostile::{entry, renderer, scene, show};
use super::{book, busy, draw, lines, one, wide};
use std::time::Duration;
use wprs_render::{drawn_effects, retry_effects, set_gpu_budget};

#[test]
fn slow_effects_are_left_out_for_the_run_without_a_strike_and_never_queue_up() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let path = book("busy");
    let effects = || scene(&[("busy", busy(BUSY)), ("fine", entry("return tap(in.uv) * 0.5;"))]);
    let s = effects();
    let key = s.loaded["busy"].key;
    set_gpu_budget(Some(Duration::from_millis(2)));
    let (mut shown, size) = (show(r, s).unwrap(), wide(r));
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
    assert!(
        left.len() == 1 && left[0].contains("busy") && left[0].contains("too slow for this graphics card"),
        "{left:?}"
    );
    assert!(!left[0].contains("stopped") && left[0].contains("ms on it") && left[0].contains("allows 2 ms"));
    assert_eq!(lines(&path, key, "1"), 0, "{}", std::fs::read_to_string(&path).unwrap_or_default());
    r.finish().unwrap();
    let fast = (0..3).map(|_| draw(r, &mut shown, size)).max().unwrap();
    r.finish().unwrap();
    assert!(fast * 2 < slowest, "frames without the busy effect took {fast:?}, with it {slowest:?}");
    assert!(!drawn_effects().contains(&key));
    set_gpu_budget(None);
    assert!(retry_effects() >= 1 && shown.view.retry(), "a view with an effect left out is built again");
    let mut shown = show(r, effects()).unwrap();
    draw(r, &mut shown, (64, 32));
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    r.finish().unwrap();
}

const BUSY: usize = 4;
