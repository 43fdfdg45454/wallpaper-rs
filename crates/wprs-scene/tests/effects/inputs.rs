use super::*;

#[test]
fn inputs_wake_only_the_effects_that_use_them() {
    probe(json!({ "id": "probe_cursor", "uses": ["cursor"], "passes": [{ "entry": "fs_a" }] }), "");
    probe(json!({ "id": "probe_audio", "uses": ["audio"], "passes": [{ "entry": "fs_a" }] }), "");
    probe(
        json!({ "id": "probe_sim", "uses": ["cursor"], "state": { "scale": 0.25, "settle": 1.0 },
        "passes": [{ "entry": "fs_a", "inputs": ["state"], "output": "state" }] }),
        "",
    );
    let mut cursor = runtime(json!([{ "effect": "probe_cursor" }]));
    assert!(cursor.uses().cursor && !cursor.uses().audio);
    assert_eq!(cursor.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    let still = cursor.pointer().follow(Some([0.2, 0.4]), 0, 0.0);
    assert!(cursor.set_pointer(still) && still.moving());
    assert_eq!(cursor.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let mut audio = runtime(json!([{ "effect": "probe_audio" }]));
    assert_eq!(audio.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(audio.set_audio(AudioFrame { silent: false, beat: true, ..AudioFrame::default() }));
    assert_eq!(audio.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    let mut sim = runtime(json!([{ "effect": "probe_sim" }]));
    assert_eq!(sim.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    let mut pointer = sim.pointer().follow(Some([0.9, 0.9]), 0, 0.0);
    sim.set_pointer(pointer);
    while pointer.moving() {
        pointer = pointer.follow(pointer.pos, 0, 0.1);
    }
    sim.set_pointer(pointer);
    let mut wakes = Vec::new();
    for _ in 0..4 {
        sim.advance(0.4);
        wakes.push(sim.frame(VIEW, Quality::Q3).wake);
    }
    assert_eq!(wakes, [Wake::Frame, Wake::Frame, Wake::Idle, Wake::Idle]);
}

#[test]
fn effect_parameters_decide_the_inputs_they_need() {
    let still = json!([{ "effect": "glitch" }, { "effect": "blur_radial" }, { "effect": "god_rays" },
        { "effect": "film_grain" }, { "effect": "shine", "params": { "speed": 0 } }]);
    let rt = runtime(still);
    let found = (rt.uses().audio, rt.uses().cursor, rt.frame(VIEW, Quality::Q3).wake);
    assert_eq!(found, (false, false, Wake::Idle));
    let cases = [
        ("glitch", json!({ "bursts": 1 }), false, false, Wake::At(1.0 / 12.0), false),
        ("glitch", json!({ "beat": true }), true, false, Wake::Idle, true),
        ("glitch", json!({ "beat": { "prop": "x" } }), false, false, Wake::Idle, false),
        ("glitch", json!({ "beat": { "script": "x" } }), true, false, Wake::Idle, false),
        ("god_rays", json!({ "follow": true }), false, true, Wake::Idle, false),
        ("depth_parallax", json!({}), false, true, Wake::Idle, false),
        ("depth_parallax", json!({ "follow": false, "orbit": true }), false, false, Wake::Frame, true),
        ("film_grain", json!({ "fps": 24 }), false, false, Wake::At(1.0 / 24.0), false),
        ("shine", json!({}), false, false, Wake::Frame, true),
        ("water_ripple", json!({}), false, true, Wake::Idle, false),
        ("water_ripple", json!({ "beat": 1 }), true, true, Wake::Idle, true),
    ];
    for (effect, params, audio, cursor, wake, audible) in cases {
        let mut rt = runtime(json!([{ "effect": effect, "params": params }]));
        let found = (rt.uses().audio, rt.uses().cursor, rt.frame(VIEW, Quality::Q3).wake);
        assert_eq!(found, (audio, cursor, wake), "{effect} {params}");
        rt.set_audio(AudioFrame { silent: false, ..AudioFrame::default() });
        assert_eq!(rt.frame(VIEW, Quality::Q3).wake == Wake::Frame, audible, "{effect} {params}");
    }
}

#[test]
fn time_effects_render_only_when_their_image_changes() {
    let wake = |effect: &str, params: serde_json::Value, t: f64, inside: bool| {
        let mut rt = runtime(json!([{ "effect": effect, "params": params }]));
        rt.advance(t);
        rt.set_pointer(PointerState {
            inside,
            presence: f32::from(u8::from(inside)),
            ..PointerState::default()
        });
        rt.frame(VIEW, Quality::Q3).wake
    };
    let near = |w: Wake, at: f64| matches!(w, Wake::At(t) if (t - at).abs() < 1e-3);
    assert!(near(wake("film_grain", json!({ "fps": 12 }), 1.02, false), 1.0 / 12.0 - 0.02));
    assert!(near(wake("shine", json!({}), 5.0, false), 3.0), "the band waits off the layer");
    assert!(near(wake("shine", json!({ "speed": -0.5 }), 1.0, false), 1.0));
    assert_eq!(wake("shine", json!({}), 9.0, false), Wake::Frame);
    assert_eq!(wake("iris", json!({}), 1.0, true), Wake::Idle, "the eye follows a resting pointer");
    assert_eq!(wake("iris", json!({}), 1.0, false), Wake::Frame);
    assert_eq!(wake("iris", json!({ "saccades": false }), 1.0, false), Wake::Idle);
    assert_eq!(wake("flow", json!({ "speed": 0 }), 1.0, false), Wake::Idle);
    assert_eq!(wake("flow", json!({}), 1.0, false), Wake::Frame);
    assert_eq!(wake("scroll", json!({ "speed": [0, 0] }), 1.0, false), Wake::Idle);
    assert_eq!(wake("scroll", json!({ "speed": [0, 0.1] }), 1.0, false), Wake::Frame);
}

#[test]
fn quality_overrides_only_lower_the_cost() {
    let rt = runtime(json!([{ "effect": "blur_motion", "params": { "samples": 4 } },
        { "effect": "clouds", "params": { "octaves": 2 } }, { "effect": "god_rays" }]));
    let slots = |q| rt.frame(VIEW, q).effects.iter().map(|e| e.slots).collect::<Vec<_>>();
    let (high, low) = (slots(Quality::Q3), slots(Quality::Q1));
    assert_eq!([high[0][2][0], high[1][8][0], high[2][6][0]], [4.0, 2.0, 12.0]);
    assert_eq!([low[0][2][0], low[1][8][0], low[2][6][0]], [4.0, 2.0, 8.0]);
}

#[test]
fn dropped_effects_need_no_inputs() {
    let rt = runtime(
        json!([{ "effect": "water_ripple" }, { "effect": "god_rays", "optional": true, "params": { "follow": true } },
        { "effect": "vignette", "params": { "intensity": { "audio": "bass" } } }]),
    );
    let needs = |q| (rt.uses_at(q).audio, rt.uses_at(q).cursor);
    assert_eq!(
        [needs(Quality::Q3), needs(Quality::Q1), needs(Quality::Q0)],
        [(true, true), (true, true), (true, false)]
    );
    let ripple = runtime(json!([{ "effect": "water_ripple", "params": { "cursor": 2 } }]));
    assert_eq!((ripple.uses_at(Quality::Q3).cursor, ripple.uses_at(Quality::Q0).cursor), (true, false));
}

#[test]
fn effects_turned_off_by_a_property_need_no_inputs() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenes/dusk/scene.json"));
    let depth = |mode: &str| Values::from([("depth".to_owned(), json!(mode))]);
    let mut rt = Runtime::new(Arc::new(Scene::from_json(&text.unwrap()).unwrap()), depth("off"));
    let mut cursor = |mode: &str| (rt.set_props(depth(mode)), rt.uses().cursor).1;
    assert_eq!(
        ["off", "pointer", "orbit", "off", "pointer"].map(&mut cursor),
        [false, true, false, false, true]
    );
    let off = runtime(json!([{ "effect": "god_rays", "enabled": false, "params": { "follow": true } }]));
    assert!(!off.uses().cursor);
}
