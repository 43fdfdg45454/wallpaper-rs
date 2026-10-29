use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("props.json")).unwrap()
}

fn runtime(text: &str) -> Runtime {
    let props = props();
    let scene = Scene::load(text, &props).unwrap();
    Runtime::new(Arc::new(scene), effective(&props, &[]))
}

fn scene(layers: serde_json::Value) -> String {
    json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
        "timelines": [{ "id": "intro", "duration": 2.0, "delay": 1.0 }],
        "assets": { "a": { "path": "a.png" }, "b": { "path": "b.png" } } })
    .to_string()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

#[test]
fn easing_curves_match_their_references() {
    let ease = |name: &str| serde_json::from_value::<Ease>(json!(name)).unwrap();
    for name in [
        "in_sine",
        "out_quad",
        "in_out_cubic",
        "in_quart",
        "out_quint",
        "in_out_expo",
        "in_circ",
        "out_back",
        "in_elastic",
        "out_bounce",
    ] {
        let e = ease(name);
        assert!(close(e.apply(0.0), 0.0) && close(e.apply(1.0), 1.0), "{name}");
        assert_eq!(serde_json::to_value(e).unwrap(), json!(name));
    }
    assert!(close(ease("in_out_cubic").apply(0.5), 0.5) && close(ease("in_quad").apply(0.5), 0.25));
    assert!(close(ease("out_bounce").apply(0.5), 0.765625));
    let css = Ease::Bezier([0.25, 0.1, 0.25, 1.0]);
    assert!((css.apply(0.5) - 0.8024).abs() < 2e-3, "{}", css.apply(0.5));
    assert!((Ease::Bezier([0.42, 0.0, 1.0, 1.0]).apply(0.25) - 0.0935).abs() < 2e-3);
    assert_eq!((Ease::Hold.apply(0.99), Ease::Step.apply(0.01)), (0.0, 1.0));
    assert!(serde_json::from_value::<Ease>(json!("sideways")).is_err());
}

#[test]
fn keys_loop_and_sample() {
    let keys: Vec<Key<f32>> = serde_json::from_value(json!([[1, 0], [3, 1, "in_quad"], [4, 0]])).unwrap();
    assert_eq!([0.0, 1.0, 2.0, 3.0, 3.5, 9.0].map(|t| sample(&keys, t)), [0.0, 0.0, 0.5, 1.0, 0.75, 0.0]);
    assert_eq!(
        [local(5.0, 2.0, Loop::Once), local(5.0, 2.0, Loop::Repeat), local(5.0, 2.0, Loop::Pingpong)],
        [2.0, 1.0, 1.0]
    );
    assert_eq!(serde_json::to_value(&keys).unwrap(), json!([[1.0, 0.0], [3.0, 1.0, "in_quad"], [4.0, 0.0]]));
}

#[test]
fn static_scenes_sleep_and_animated_ones_ask_for_frames() {
    let still = runtime(&scene(
        json!([{ "id": "bg", "type": "image", "image": "a" }, { "id": "c", "type": "solid", "color": "#ff0000" }]),
    ));
    let frame = still.frame(VIEW, Quality::Q3);
    assert_eq!(
        (frame.wake, frame.draws.len(), frame.draws.iter().any(|d| d.dynamic)),
        (Wake::Idle, 2, false)
    );
    let spin = scene(
        json!([{ "id": "s", "type": "solid", "color": "#ff0000", "transform": { "rotation": { "keys": [[0, 0], [1, 90]] } } }]),
    );
    let mut spin = runtime(&spin);
    assert!(spin.frame(VIEW, Quality::Q3).draws[0].dynamic);
    assert_eq!(spin.frame(VIEW, Quality::Q3).wake, Wake::Frame);
    spin.seek(1.5);
    assert_eq!(spin.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    let waves =
        scene(json!([{ "id": "w", "type": "image", "image": "a", "effects": [{ "effect": "waves" }] }]));
    assert_eq!(runtime(&waves).frame(VIEW, Quality::Q0).wake, Wake::Frame);
    let blur = scene(
        json!([{ "id": "b", "type": "image", "image": "a", "effects": [{ "effect": "blur", "optional": true }] }]),
    );
    let blur = runtime(&blur);
    assert_eq!(
        (blur.frame(VIEW, Quality::Q3).effects.len(), blur.frame(VIEW, Quality::Q0).effects.len()),
        (1, 0)
    );
    assert_eq!(blur.frame(VIEW, Quality::Q2).effects[0].scale, 0.5);
}

#[test]
fn timelines_follow_the_injected_clock() {
    let text = scene(json!([{ "id": "s", "type": "solid", "color": "#ffffff",
        "opacity": { "timeline": "intro", "keys": [[0, 0], [2, 1]] } }]));
    let mut rt = runtime(&text);
    let opacity = |rt: &Runtime| rt.frame(VIEW, Quality::Q3).draws.first().map_or(0.0, |d| d.opacity);
    assert_eq!(opacity(&rt), 0.0);
    assert!(matches!(rt.wake(), Wake::At(t) if close(t as f32, 1.0)));
    for (dt, expected) in [(0.5, 0.0), (1.0, 0.25), (0.5, 0.5), (1.0, 1.0), (5.0, 1.0)] {
        for _ in 0..(dt * 10.0) as usize {
            rt.advance(0.1);
        }
        assert!(close(opacity(&rt), expected), "{} {}", rt.time(), opacity(&rt));
    }
    assert_eq!(rt.wake(), Wake::Idle);
    let mut late = runtime(&text);
    late.advance(1.5);
    assert!(close(opacity(&late), 0.25) && late.wake() == Wake::Frame, "a long wait catches up");
    rt.set_script(ScriptOutputs {
        timelines: vec![("intro".into(), TimelineCommand::Seek(1.0))],
        ..Default::default()
    });
    assert!(close(opacity(&rt), 0.5));
    let triggered = text.replace("\"delay\":1.0", "\"delay\":0.0,\"trigger\":\"prop:soft\"");
    let mut rt = runtime(&triggered);
    rt.advance(0.1);
    assert_eq!(opacity(&rt), 0.0);
    let mut values = rt.props().clone();
    values.insert("soft".into(), json!(false));
    assert!(rt.set_props(values.clone()) && !rt.set_props(values));
    rt.advance(0.1);
    assert!(close(opacity(&rt), 0.05));
    let mut rt = runtime(&text.replace("\"delay\":1.0", "\"delay\":0.0,\"trigger\":\"beat\""));
    assert!(rt.uses().audio && !rt.uses().cursor);
    assert!(rt.set_audio(AudioFrame { beat: true, silent: false, ..AudioFrame::default() }));
    rt.advance(0.1);
    assert!(close(opacity(&rt), 0.05));
}

#[test]
fn values_follow_properties_audio_cursor_and_the_clock() {
    let text = scene(json!([
        { "id": "img", "type": "image", "image": { "prop": "style", "map": { "night": "a", "dawn": "b" } } },
        { "id": "lvl", "type": "solid", "color": { "prop": "text_color" }, "opacity": { "audio": "bass", "clamp": [0, 1] } },
        { "id": "tx", "type": "text", "text": { "clock": "HH:mm:ss a" }, "anchor": "bottom_right", "transform": { "position": [-10, -5] } },
        { "id": "px", "type": "solid", "color": "#000000", "parallax": 0.1, "size": [10, 10] }
    ]));
    let mut rt = runtime(&text);
    rt.set_wall(13.0 * 3600.0 + 5.0 * 60.0 + 9.5);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(frame.draws.len(), 3);
    assert!(matches!(&frame.draws[0].content, Content::Image { asset, .. } if asset == "a"));
    assert!(matches!(&frame.draws[1].content, Content::Text { text, .. } if text == "13:05:09 PM"));
    assert_eq!(frame.draws[1].world.apply([0.0, 0.0]), [90.0, 45.0]);
    assert_eq!(frame.draws[1].origin, [1.0, 1.0]);
    assert!(matches!(frame.wake, Wake::At(t) if close(t as f32, 0.5)));
    let mut values = rt.props().clone();
    values.insert("style".into(), json!("dawn"));
    values.insert("text_color".into(), json!("#ff0000"));
    rt.set_props(values);
    let mut audio = AudioFrame { bass: 0.7, silent: false, ..AudioFrame::default() };
    assert!(rt.set_audio(audio.clone()) && !rt.set_audio(audio.clone()));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(matches!(&frame.draws[0].content, Content::Image { asset, .. } if asset == "b"));
    assert!(
        matches!(&frame.draws[1].content, Content::Solid { color, .. } if *color == Color([1.0, 0.0, 0.0, 1.0]))
    );
    assert!(close(frame.draws[1].opacity, 0.7) && frame.wake == Wake::Frame);
    audio.silent = true;
    rt.set_audio(audio);
    let pointer = PointerState { pos: Some([1.0, 0.5]), smooth: [1.0, 0.5], ..PointerState::default() };
    assert!(rt.set_pointer(pointer));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(frame.draws[3].world.apply([0.0, 0.0]), [55.0, 25.0]);
}

#[test]
fn the_pointer_settles_and_hidden_mattes_stay_in_the_draw_list() {
    let step = |p: PointerState, pos| p.follow(pos, 1, 1.0 / 60.0);
    let mut pointer = (0..6).fold(PointerState::default(), |p, _| step(p, Some([1.0, 0.0])));
    assert!(pointer.moving() && pointer.inside && pointer.smooth[0] > 0.7 && pointer.velocity[0] > 0.0);
    pointer = (0..60).fold(pointer, |p, _| step(p, Some([1.0, 0.0])));
    assert_eq!((pointer.smooth, pointer.velocity, pointer.moving()), ([1.0, 0.0], [0.0; 2], false));
    let left = step(pointer, None);
    assert!(!left.inside && left.smooth[0] < 1.0 && left.moving() && left.presence > 0.9);
    let gone = (0..60).fold(left, |p, _| step(p, None));
    assert_eq!((gone.smooth, gone.presence, gone.moving()), ([0.5; 2], 0.0, false));
    let text = scene(json!([
        { "id": "mattes", "type": "group", "visible": false, "layers": [{ "id": "m", "type": "solid", "color": "#ffffff" }] },
        { "id": "cut", "type": "solid", "color": "#ff0000", "mask": { "layer": "m" } },
        { "id": "gone", "type": "solid", "color": "#00ff00", "visible": false }
    ]));
    let rt = runtime(&text);
    let frame = rt.frame(VIEW, Quality::Q3);
    let drawn: Vec<_> = frame.draws.iter().map(|d| (d.layer.id.as_str(), d.hidden)).collect();
    assert_eq!(drawn, [("mattes", true), ("m", false), ("cut", false)]);
}

#[test]
fn properties_check_and_clamp_values() {
    let props = props();
    assert!(props.values().all(|p| p.check().is_ok()));
    let check = |id: &str, v: serde_json::Value| props[id].value(&v);
    assert_eq!(check("depth", json!(3.0)), Ok(json!(1.0)));
    assert_eq!(check("density", json!(55.4)), Ok(json!(60)));
    assert_eq!(check("text_color", json!("#A0B0C0")), Ok(json!("#a0b0c0")));
    assert!(check("text_color", json!("#a0b0c0ff")).is_err());
    assert!(check("style", json!("noon")).is_err() && check("style", json!("dawn")).is_ok());
    assert!(check("soft", json!("yes")).is_err() && check("photo", json!("relative.png")).is_err());
    assert_eq!(check("photo", json!(null)), Ok(json!(null)));
    let presets: Vec<Preset> =
        serde_json::from_value(json!([{ "id": "calm", "values": { "density": 20, "depth": 0.2 } }])).unwrap();
    assert_eq!(check_presets(&props, &presets), Ok(()));
    let bad: Vec<Preset> =
        serde_json::from_value(json!([{ "id": "x", "values": { "photo": "/a.png" } }])).unwrap();
    assert!(check_presets(&props, &bad).is_err());
    let output = BTreeMap::from([("depth".to_string(), json!(0.9))]);
    let global = BTreeMap::from([("depth".to_string(), json!(0.1)), ("soft".to_string(), json!(false))]);
    let values = effective(&props, &[&output, &global]);
    assert_eq!(
        (&values["depth"], &values["soft"], &values["style"]),
        (&json!(0.9), &json!(false), &json!("night"))
    );
    let old = BTreeMap::from([
        ("depth".to_string(), json!(3.0)),
        ("soft".to_string(), json!("yes")),
        ("gone".to_string(), json!(1)),
    ]);
    let values = effective(&props, &[&old, &global]);
    assert_eq!((&values["depth"], &values["soft"]), (&json!(0.1), &json!(false)));
    assert_eq!(
        stale(&props, &[&old, &global]),
        [
            "depth no longer accepts 3.0; the default is used",
            "gone no longer exists; the default is used",
            "soft no longer accepts \"yes\"; the default is used"
        ]
    );
    let tier = Tier::detect(&wprs_scene::Gpu { software: true, compute: true, ..Default::default() });
    assert_eq!((tier, tier.max_fps(true, 60, None), tier.quality(false)), (Tier::T0, 15, Quality::Q0));
    let sample = Scene::from_json(include_str!("sample.json")).unwrap();
    assert!(sample.cost(Quality::Q0) < sample.cost(Quality::Q3));
    assert_eq!(Quality::fit(&sample, Tier::T3, Quality::Q3), Quality::Q3);
}

#[test]
fn forms_order_localize_and_hide_properties() {
    let mut props = props();
    let extra: BTreeMap<String, Property> = serde_json::from_value(json!({
        "look": { "type": "group", "label": { "en": "Look", "es": "Aspecto" }, "order": -1 },
        "glow": { "type": "slider", "group": "look", "min": 0, "max": 1, "step": 0.1, "default": 0.5,
                  "label": { "en": "Glow", "es-ES": "Brillo" } }
    }))
    .unwrap();
    props.extend(extra);
    let presets: Vec<Preset> = serde_json::from_value(
        json!([{ "id": "calm", "label": { "en": "Calm", "es": "Calma" }, "values": {} }]),
    )
    .unwrap();
    let values = effective(&props, &[&BTreeMap::from([("soft".to_string(), json!(false))])]);
    let (fields, labels) = form(&props, &presets, &values, &["es_ES.UTF-8".to_string()]);
    let keys: Vec<_> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(
        keys,
        ["look", "glow", "density", "depth", "photo", "show_clock", "soft", "style", "text_color"]
    );
    assert_eq!(
        (fields[0].label.as_str(), fields[1].label.as_str(), &fields[1].value),
        ("Aspecto", "Brillo", &json!(0.5))
    );
    let find = |key: &str| fields.iter().find(|f| f.key == key).unwrap();
    assert!(!find("show_clock").visible && find("soft").visible && find("density").integer);
    let style = find("style");
    let options: Vec<_> =
        style.options.iter().map(|o| (o.value.as_str().unwrap(), o.label.as_str())).collect();
    assert_eq!((style.label.as_str(), options), ("Style", vec![("night", "Night"), ("dawn", "dawn")]));
    assert_eq!(
        (labels[0].label.as_str(), form(&props, &presets, &values, &[]).1[0].label.as_str()),
        ("Calma", "Calm")
    );
    let broken = |extra: serde_json::Value| {
        let mut props = props.clone();
        props.extend(serde_json::from_value::<BTreeMap<String, Property>>(extra).unwrap());
        check_presets(&props, &[]).is_err()
    };
    assert!(!broken(json!({})));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "group": "soft" } })));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "visible_if": { "nothing": true } } })));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "visible_if": { "soft": 3 } } })));
    let mut shown = props.clone();
    shown.extend(
        serde_json::from_value::<BTreeMap<String, Property>>(json!({
            "a": { "type": "bool", "default": true, "visible_if": { "density": 120 } },
            "b": { "type": "bool", "default": true, "visible_if": { "density": 120.0, "text_color": "#FFFFFF" } },
            "c": { "type": "bool", "default": true, "visible_if": { "depth": 0.4 } },
            "d": { "type": "bool", "default": true, "visible_if": { "depth": 1 } }
        }))
        .unwrap(),
    );
    assert_eq!(check_presets(&shown, &[]), Ok(()));
    let fields = form(&shown, &[], &effective(&shown, &[]), &[]).0;
    let visible = |key: &str| fields.iter().any(|f| f.key == key && f.visible);
    assert_eq!(["a", "b", "c", "d"].map(visible), [true, true, true, false]);
}

#[test]
fn declared_inputs_limit_what_a_scene_receives() {
    let layers = json!([{ "id": "glow", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "opacity": { "audio": "bass" }, "parallax": 0.5 }]);
    let mut rt = runtime(&scene(layers));
    assert!(rt.uses().audio && rt.uses().cursor);
    rt.declare(Uses { audio: false, cursor: true, clicks: false, scripts: true, media: false });
    assert!(!rt.uses().audio && rt.uses().cursor && !rt.uses_at(Quality::Q0).audio);
    let loud = AudioFrame { bass: 1.0, silent: false, ..AudioFrame::default() };
    assert!(!rt.set_audio(loud), "an input the package did not declare never wakes it");
}
