use super::hostile::{entry, frame, renderer, scene, show};
use super::{book, busy, draw, lines, one, wide};
use std::time::Duration;
use wprs_render::drawn_effects;

#[test]
fn a_view_that_stopped_drawing_is_not_blamed_for_a_lost_device() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("blame");
    let (left, right) = (
        scene(&[("left", entry("return tap(in.uv).gbra;"))]),
        scene(&[("right", entry("return tap(in.uv).brga;"))]),
    );
    let b = right.loaded["right"].key;
    let (mut left, mut right) = (show(r, left).unwrap(), show(r, right).unwrap());
    for _ in 0..4 {
        frame(r, &mut right).unwrap();
    }
    std::thread::sleep(Duration::from_secs(3));
    frame(r, &mut left).unwrap();
    let drawn = drawn_effects();
    assert!(!drawn.contains(&b), "{drawn:?} {b}");
}

#[test]
fn frames_still_on_the_gpu_are_blamed_until_they_finish() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("flying");
    let s = scene(&[("flying", busy(2))]);
    let key = s.loaded["flying"].key;
    let (mut shown, size) = (show(r, s).unwrap(), wide(r));
    for _ in 0..4 {
        draw(r, &mut shown, size);
    }
    let flying = shown.view.stats().in_flight;
    assert!(flying == 0 || drawn_effects().contains(&key), "{flying} frames in flight but none blamed");
    r.finish().unwrap();
    assert!(!drawn_effects().contains(&key));
}

#[test]
fn a_still_view_ends_its_trial_soon_after_its_first_frame() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let path = book("still");
    let s = scene(&[("still", entry("return tap(in.uv).rbga;"))]);
    let key = s.loaded["still"].key;
    let mut shown = show(r, s).unwrap();
    assert_eq!(lines(&path, key, "trying"), 1);
    frame(r, &mut shown).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(lines(&path, key, "trying"), 0, "a view drawn once kept its effect on trial");
}
