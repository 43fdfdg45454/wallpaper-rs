use super::*;

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
