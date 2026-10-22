use super::*;
use wprs_scene::AudioFrame;

#[test]
fn glitch_waits_for_bursts_or_beats() {
    let Some(plain) = Fx::chain(json!([])).shot() else { return };
    let Some(off) = Fx::new("glitch", json!({})).shot() else { return };
    assert_eq!(off.diff(&plain), 0.0);
    let bursts = json!({ "bursts": 10, "threshold": 0, "shift": 16, "split": 0, "jitter": 0 });
    let Some(burst) = Fx::new("glitch", bursts).shot() else { return };
    assert!(burst.diff(&plain) > 2.0, "{}", burst.diff(&plain));
    let Some(mut run) = Fx::new("glitch", json!({ "beat": true, "threshold": 0 })).run() else { return };
    assert_eq!(run.shot().diff(&plain), 0.0);
    run.runtime.set_audio(AudioFrame { beat: true, silent: false, ..AudioFrame::default() });
    assert!(run.shot().diff(&plain) > 2.0);
    run.runtime.set_audio(AudioFrame { silent: false, ..AudioFrame::default() });
    run.runtime.advance(1.0 / 30.0);
    assert!(run.shot().diff(&plain) > 2.0, "a beat glitch lasts its length");
    run.runtime.advance(0.25);
    assert_eq!(run.shot().diff(&plain), 0.0);
}
