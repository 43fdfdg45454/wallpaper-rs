use super::*;
use wprs_scene::{AudioFrame, Wake};

#[test]
fn pulse_beats_brightness_color_or_opacity_and_follows_a_band() {
    let at = |params: Value, t: f64| {
        let fx = Fx::new("pulse", params).input(solid([200; 3])).background("#0000ff");
        let mut run = fx.space("srgb").run()?;
        run.runtime.seek(t);
        Some(run.shot())
    };
    let Some(top) = at(json!({ "speed": 1, "bounds": [0.25, 1] }), 0.0) else { return };
    let Some(low) = at(json!({ "speed": 1, "bounds": [0.25, 1] }), 0.5) else { return };
    top.check(&[((10, 10), [200; 3])]);
    low.check(&[((10, 10), [50; 3])]);
    let Some(tinted) = at(json!({ "what": 1, "speed": 1, "tint_high": "#ff0000" }), 0.0) else { return };
    tinted.check(&[((10, 10), [200, 0, 0])]);
    let Some(faded) = at(json!({ "what": 2, "speed": 1, "bounds": [0, 1] }), 0.5) else { return };
    faded.check(&[((10, 10), [0, 0, 255])]);
    let Some(mut heard) = Fx::new("pulse", json!({ "speed": 0, "audio_band": 3 })).input(solid([200; 3])).run() else {
        return;
    };
    let quiet = heard.shot();
    let mut bands = [0.0; 64];
    bands[2] = 1.0;
    heard.runtime.set_audio(AudioFrame { average: bands, silent: false, ..AudioFrame::default() });
    let loud = heard.shot();
    assert!(loud.mean()[0] > quiet.mean()[0] + 50.0 && heard.runtime.wake() == Wake::Frame);
    Shot::sheet(&[top, low, tinted, faded, quiet, loud]).looks_like("pulse");
}
