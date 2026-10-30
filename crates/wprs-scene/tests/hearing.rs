use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

fn runtime(opacities: &[Json], audio: Option<Json>) -> Runtime {
    let layer = |(i, o): (usize, &Json)| json!({ "id": format!("l{i}"), "type": "solid", "color": "#ffffff", "opacity": o });
    let layers: Vec<Json> = opacities.iter().enumerate().map(layer).collect();
    let mut text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers });
    if let Some(a) = audio {
        text["audio"] = a;
    }
    let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
    assert!(scene.validate(&BTreeMap::new()).is_empty(), "{:?}", scene.validate(&BTreeMap::new()));
    Runtime::new(Arc::new(scene), Values::new())
}

fn read(rt: &Runtime) -> Vec<f32> {
    let frame = rt.frame(VIEW, Quality::Q3);
    let n = rt.scene.layers.len();
    (0..n)
        .map(|i| frame.draws.iter().find(|d| d.layer.id == format!("l{i}")).map_or(0.0, |d| d.opacity))
        .collect()
}

fn ramp() -> AudioFrame {
    let mut a = AudioFrame { silent: false, ..AudioFrame::default() };
    for b in 0..BANDS {
        (a.left[b], a.right[b]) = (b as f32 / 63.0, 1.0 - b as f32 / 63.0);
        a.average[b] = a.left[b];
    }
    a
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn ranges_of_bands_reduce_by_mean_or_max_on_a_channel() {
    let bindings = [
        json!({ "audio": [0, 3], "of": 16, "reduce": "max" }),
        json!({ "audio": [0, 3], "of": 16 }),
        json!({ "audio": 2, "of": 16 }),
        json!({ "audio": [60, 63], "channel": "right" }),
        json!({ "audio": [1, 1], "of": 32, "channel": "left", "reduce": "max" }),
    ];
    let mut rt = runtime(&bindings, None);
    rt.set_audio(ramp());
    let got = read(&rt);
    let want = [15.0 / 63.0, 7.5 / 63.0, 9.5 / 63.0, 1.0 - 61.5 / 63.0, 3.0 / 63.0];
    assert!(got.iter().zip(want).all(|(g, w)| close(*g, w)), "{got:?} {want:?}");
}

#[test]
fn bounds_and_an_exponent_shape_sound_and_the_pointer() {
    let bindings = [
        json!({ "audio": "bass", "bounds": [0.8, 1] }),
        json!({ "audio": "bass", "bounds": [0.8, 1], "exponent": 2 }),
        json!({ "audio": "bass", "bounds": [0.95, 1] }),
        json!({ "cursor": "x", "bounds": [0.5, 1], "exponent": 2, "scale": 0.5, "offset": 0.5 }),
    ];
    let mut rt = runtime(&bindings, None);
    rt.set_audio(AudioFrame { bass: 0.9, silent: false, ..AudioFrame::default() });
    rt.set_pointer(PointerState { pos: Some([0.75, 0.5]), smooth: [0.75, 0.5], ..PointerState::default() });
    let got = read(&rt);
    assert!(close(got[0], 0.5) && close(got[1], 0.25) && got[2] == 0.0 && close(got[3], 0.625), "{got:?}");
}

#[test]
fn uneven_smoothing_rises_fast_and_falls_slowly() {
    let mut rt = runtime(&[json!({ "audio": "bass", "smooth": { "up": 0.01, "down": 1 } })], None);
    let loud = |bass| AudioFrame { bass, silent: false, ..AudioFrame::default() };
    rt.set_audio(loud(0.0));
    read(&rt);
    rt.advance(1.0 / 30.0);
    rt.set_audio(loud(1.0));
    let up = read(&rt)[0];
    rt.advance(1.0 / 30.0);
    rt.set_audio(loud(0.0));
    let down = read(&rt)[0];
    assert!(up > 0.95 && down > 0.9 && down < up, "{up} {down}");
}

#[test]
fn a_scene_sets_its_own_attack_release_and_gain() {
    let synthetic = |raw: f32, loud: f32| {
        let mut a = AudioFrame { silent: false, loudness: loud, ..AudioFrame::default() };
        (a.raw, a.loud) = ([[raw; BANDS]; 2], [[loud; BANDS]; 2]);
        a
    };
    let bindings =
        [json!({ "audio": "bass" }), json!({ "audio": "level" }), json!({ "audio": 40, "channel": "left" })];
    for (gain, target) in [("auto", 1.0), ("fixed", 0.2)] {
        let mut rt = runtime(&bindings, Some(json!({ "attack": 0.5, "release": 0.05, "gain": gain })));
        rt.set_audio(synthetic(1.0, 0.2));
        rt.advance(0.1);
        rt.set_audio(synthetic(1.0, 0.2));
        let rise = target * (1.0 - (-0.2f32).exp());
        let got = read(&rt);
        assert!(close(got[0], rise) && close(got[2], rise), "{gain}: {got:?} {rise}");
        assert!(close(got[1], 0.2 * (1.0 - (-0.2f32).exp())), "{gain}: {got:?}");
        rt.advance(0.1);
        rt.set_audio(synthetic(0.0, 0.0));
        assert!(read(&rt)[0] < rise * 0.2, "{gain}: the release is quick");
    }
    let plain = runtime(&bindings, None);
    assert!(!plain.scene.features().contains("audio_response"));
}
