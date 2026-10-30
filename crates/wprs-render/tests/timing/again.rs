use super::hostile::{entry, renderer, scene, show, unchecked};
use super::{book, one, slow_build};
use std::time::{Duration, Instant};
use wprs_render::{drawn_effects, retry_effects, set_build_timeout};

#[test]
fn trying_again_builds_effects_that_took_too_long_to_build() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("again");
    set_build_timeout(Duration::from_millis(100));
    let mut s = scene(&[("slow", entry("return tap(in.uv);"))]);
    let key = unchecked(&mut s, "slow", &slow_build());
    let mut shown = show(r, s).unwrap();
    set_build_timeout(Duration::from_secs(10));
    let left = shown.view.left_out();
    assert!(left.len() == 1 && left[0].contains("to build"), "{left:?}");
    retry_effects();
    wprs_scene::forgive_strikes().unwrap();
    assert!(shown.view.retry(), "the view was not built again");
    assert!(shown.view.left_out().is_empty());
    let started = Instant::now();
    while drawn_effects().contains(&key) && started.elapsed() < Duration::from_secs(300) {
        std::thread::sleep(Duration::from_millis(50));
    }
}
