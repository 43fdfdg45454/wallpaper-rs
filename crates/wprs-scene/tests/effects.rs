use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 32.0];

fn probe(json: serde_json::Value, wgsl: &str) -> &'static Descriptor {
    register(Descriptor::parse(&json.to_string(), wgsl).unwrap()).unwrap()
}

fn scene(effects: serde_json::Value) -> Scene {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "a": { "path": "a.png" }, "grade": { "path": "grade.cube" } },
        "layers": [{ "id": "l", "type": "image", "image": "a", "effects": effects }] });
    Scene::from_json(&text.to_string()).unwrap()
}

fn runtime(effects: serde_json::Value) -> Runtime {
    Runtime::new(Arc::new(scene(effects)), Values::new())
}

#[test]
fn lut_files_parse_within_limits() {
    let corners = (0..8).map(|i| format!("{} {} {}\n", i & 1, (i >> 1) & 1, i >> 2)).collect::<String>();
    let head = "TITLE \"probe\"\n# comment\nLUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n";
    let lut = Lut::parse(format!("{head}{corners}").as_bytes()).unwrap();
    assert_eq!(
        (lut.size, lut.data.len(), lut.data[1], lut.data[6]),
        (2, 8, [1.0, 0.0, 0.0], [0.0, 1.0, 1.0])
    );
    let short: String = corners.lines().take(7).map(|l| format!("{l}\n")).collect();
    let cases = [
        (corners.clone(), LutError::Size),
        (format!("LUT_3D_SIZE 1\n{corners}"), LutError::Size),
        (format!("LUT_3D_SIZE 66\n{corners}"), LutError::Size),
        (format!("LUT_3D_SIZE 2\n{short}"), LutError::Count(7, 8)),
        (format!("LUT_3D_SIZE 2\n{corners}0 0 0\n"), LutError::Count(9, 8)),
        (format!("LUT_1D_SIZE 4\n{corners}"), LutError::Line(1)),
        (format!("LUT_3D_SIZE 2\nDOMAIN_MAX 2 2 2\n{corners}"), LutError::Line(2)),
        (format!("LUT_3D_SIZE 2\nNaN 0 0\n{short}"), LutError::Line(2)),
    ];
    for (text, error) in cases {
        assert_eq!(Lut::parse(text.as_bytes()), Err(error), "{text}");
    }
    assert_eq!(Lut::parse(&[0xff, 0xfe]), Err(LutError::Text));
    assert_eq!(Lut::parse(&vec![b' '; MAX_LUT_BYTES + 1]), Err(LutError::TooLarge));
}

#[test]
fn descriptors_are_checked_and_tuned_per_quality() {
    let tuned = probe(
        json!({ "id": "probe_tune", "params": [{ "id": "taps", "type": "float", "default": 16 }],
            "passes": [{ "entry": "fs_a" }], "cost": { "taps": 8 },
            "quality": { "q1": { "params": { "taps": 8 } }, "q0": { "params": { "taps": 4 }, "scale": 0.5 } } }),
        "",
    );
    let at = |q| (tuned.tuned(q, "taps").cloned(), tuned.scale_at(q));
    assert_eq!([at(Quality::Q3), at(Quality::Q2)], [(None, 1.0), (None, 1.0)]);
    assert_eq!(
        [at(Quality::Q1), at(Quality::Q0)],
        [(Some(Param::Number(8.0)), 1.0), (Some(Param::Number(4.0)), 0.5)]
    );
    assert_eq!(descriptor("probe_tune"), Some(tuned));
    assert!(register(tuned.clone()).is_ok());
    assert!(register(Descriptor { cost: Cost::default(), ..tuned.clone() }).is_err());
    assert!(register(Descriptor { wgsl: "changed".into(), ..descriptor("blur").unwrap().clone() }).is_err());
    let rt = runtime(json!([{ "effect": "probe_tune", "params": { "taps": 32 } }]));
    let slots = |q| rt.frame(VIEW, q).effects.first().map(|e| (e.slots[0][0], e.scale));
    assert_eq!(
        [slots(Quality::Q3), slots(Quality::Q2), slots(Quality::Q0)],
        [Some((32.0, 1.0)), Some((32.0, 0.5)), None]
    );
    let broken = [
        (json!({ "passes": [{ "entry": "fs_a", "inputs": ["pass:0"] }] }), "unknown input pass:0"),
        (json!({ "passes": [{ "entry": "fs_a", "output": "state" }] }), "missing state"),
        (json!({ "passes": [{ "entry": "fs_a", "inputs": ["asset:map"] }] }), "unknown input asset:map"),
        (json!({ "fuse": "uv" }), "fn uv_probe_bad("),
        (json!({ "passes": [{ "entry": "fs_a" }], "reach": "size" }), "reach"),
        (json!({ "passes": [], "params": [{ "id": "k", "type": "texture", "default": 1 }] }), "default of k"),
        (json!({ "passes": [{ "entry": "fs_a" }], "quality": { "q0": { "params": { "k": 1 } } } }), "sets k"),
    ];
    for (mut json, problem) in broken {
        json["id"] = json!("probe_bad");
        let d = Descriptor::parse(&json.to_string(), "").unwrap();
        assert!(d.problems().iter().any(|p| p.contains(problem)), "{problem}: {:?}", d.problems());
        assert!(register(d).is_err());
    }
}

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

#[test]
fn asset_parameters_name_assets_of_their_kind() {
    probe(
        json!({ "id": "probe_assets", "passes": [{ "entry": "fs_a", "inputs": ["previous", "asset:depth"] }],
            "params": [{ "id": "depth", "type": "texture", "default": "" }, { "id": "grade", "type": "lut", "default": "" }] }),
        "",
    );
    let issues = |params: serde_json::Value| {
        scene(json!([{ "effect": "probe_assets", "params": params }])).validate(&BTreeMap::new())
    };
    assert_eq!(issues(json!({ "depth": "a", "grade": "grade" })), []);
    let path = |p: &str| format!("/layers/0/effects/0/params/{p}");
    let unknown = Issue::Unknown { path: path("depth"), what: "asset", id: "gone".into() };
    assert_eq!(issues(json!({ "depth": "gone" })), [unknown]);
    assert_eq!(
        issues(json!({ "grade": "a", "depth": "grade" })),
        [Issue::Type(path("depth")), Issue::Type(path("grade"))]
    );
    assert_eq!(issues(json!({ "depth": { "prop": "x" } })).last(), Some(&Issue::Type(path("depth"))));
}

#[test]
fn built_in_effects_report_their_cost() {
    for d in builtin() {
        assert!(d.units().is_finite() && d.units() > 0.0, "{}", d.id);
    }
    let class = |id: &str| descriptor(id).map(Descriptor::class);
    let light = ["dissolve", "flow", "iris", "lens_distort", "perspective", "reflection", "scroll", "shake"];
    assert_eq!(light.map(class), [Some(Class::Light); 8]);
    assert_eq!(["sway", "twirl", "water_rings"].map(class), [Some(Class::Light); 3]);
    assert_eq!(["clouds", "water_ripple"].map(class), [Some(Class::Medium); 2]);
    let clouds = descriptor("clouds").unwrap();
    let octaves = |q| clouds.tuned(q, "octaves").cloned();
    assert_eq!(
        [Quality::Q3, Quality::Q2, Quality::Q1].map(octaves),
        [None, Some(Param::Number(4.0)), Some(Param::Number(3.0))]
    );
    let still = ["twirl", "lens_distort", "perspective", "dissolve"];
    assert!(still.iter().all(|id| !descriptor(id).unwrap().animates()));
    assert_eq!(descriptor("shake").unwrap().pack(|_| None)[0][0], 0.0);
}

#[test]
fn perspective_solves_the_homography_of_its_corners() {
    let d = descriptor("perspective").unwrap();
    let corners = [[0.2, 0.1], [0.9, 0.0], [1.0, 1.0], [0.0, 0.8]];
    let names = ["top_left", "top_right", "bottom_right", "bottom_left"];
    let slots = d.pack(|id| names.iter().position(|n| *n == id).map(|i| Param::Vec2(corners[i])));
    let unit = |[x, y]: [f32; 2]| {
        let h = [4, 5, 6].map(|r| slots[r][0] * x + slots[r][1] * y + slots[r][2]);
        [h[0] / h[2], h[1] / h[2]]
    };
    for (corner, expected) in corners.into_iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
        let found = unit(corner);
        assert!(found.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-4), "{found:?} {expected:?}");
    }
    let mut broken = d.clone();
    broken.params.pop();
    assert!(broken.problems().iter().any(|p| p.contains("homography")));
}

#[test]
fn ripples_wake_on_presses_and_settle() {
    let mut rt = runtime(json!([{ "effect": "water_ripple" }]));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    let press = PointerState {
        pos: Some([0.5; 2]),
        smooth: [0.5; 2],
        buttons: 1,
        inside: true,
        presence: 1.0,
        ..Default::default()
    };
    assert!(rt.set_pointer(press) && !press.moving());
    let mut wakes = Vec::new();
    for _ in 0..3 {
        wakes.push(rt.frame(VIEW, Quality::Q3).wake);
        rt.advance(1.9);
    }
    assert_eq!(wakes, [Wake::Frame, Wake::Frame, Wake::Frame]);
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(rt.set_pointer(PointerState { buttons: 0, ..press }));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(rt.set_pointer(PointerState { buttons: 2, ..press }));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
}
