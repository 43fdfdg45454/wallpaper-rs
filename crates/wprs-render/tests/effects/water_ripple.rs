use super::*;
use wprs_scene::{AudioFrame, Wake};

fn checker() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| Rgba(if (x / 4 + y / 4) % 2 == 0 { [255; 4] } else { [0, 0, 0, 255] }))
}

#[test]
fn water_ripples_spread_alike_at_any_frame_rate() {
    let ripple = |fps: f64| {
        let mut run = Fx::new("water_ripple", json!({ "radius": 2 })).input(checker()).run()?;
        let rest = run.shot();
        run.pointer([0.5; 2]);
        run.runtime.set_pointer(PointerState { buttons: 1, ..*run.runtime.pointer() });
        for _ in 0..(0.1 * fps).round() as usize {
            run.runtime.advance(1.0 / fps);
            run.shot();
        }
        Some((run.shot(), rest))
    };
    let (Some((fast, rest)), Some((slow, _))) = (ripple(60.0), ripple(30.0)) else { return };
    assert!(fast.diff(&rest) > 1.0 && slow.diff(&fast) < 0.2, "{} {}", fast.diff(&rest), slow.diff(&fast));
}

#[test]
fn water_ripple_spreads_from_clicks_and_beats_then_settles() {
    let Some(mut run) = Fx::new("water_ripple", json!({ "beat": 1 })).input(checker()).run() else { return };
    let rest = run.shot();
    rest.check(&[((1, 1), [255; 3]), ((5, 1), [0; 3])]);
    run.pointer([0.5; 2]);
    run.runtime.set_pointer(PointerState { buttons: 1, ..*run.runtime.pointer() });
    let mut changes = Vec::new();
    for _ in 0..12 {
        run.runtime.advance(1.0 / 30.0);
        changes.push(run.shot().diff(&rest));
    }
    assert!(changes.iter().all(|d| *d > 1.0), "{changes:?}");
    assert_eq!((run.runtime.wake(), run.stats().created), (Wake::Frame, 0));
    while run.runtime.wake() == Wake::Frame {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
    }
    assert!(run.runtime.time() < 5.0 && run.shot().diff(&rest) < 0.05, "{}", run.shot().diff(&rest));
    run.runtime.set_audio(AudioFrame { beat: true, silent: false, ..AudioFrame::default() });
    run.runtime.advance(1.0 / 30.0);
    run.shot();
    run.runtime.advance(1.0 / 30.0);
    assert!(run.shot().diff(&rest) > 0.5 && run.runtime.wake() == Wake::Frame);
}
