use super::*;

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
    let sample = Scene::from_json(include_str!("../sample.json")).unwrap();
    assert!(sample.cost(Quality::Q0) < sample.cost(Quality::Q3));
    assert_eq!(Quality::fit(&sample, Tier::T3, Quality::Q3), Quality::Q3);
}

#[test]
fn a_slider_whose_end_is_off_its_step_grid_keeps_that_end() {
    let slider =
        json!({ "type": "slider", "label": "P", "min": 0, "max": 0.175, "step": 0.01, "default": 0.1 });
    let props: BTreeMap<String, Property> = serde_json::from_value(json!({ "parallax": slider })).unwrap();
    let set = |v: f64| props["parallax"].value(&json!(v));
    assert_eq!([set(0.175), set(0.2), set(0.0), set(0.123)], [0.175, 0.175, 0.0, 0.12].map(|v| Ok(json!(v))));
    let kept = BTreeMap::from([("parallax".to_string(), json!(0.175))]);
    assert!(stale(&props, &[&kept]).is_empty());
    assert_eq!(effective(&props, &[&kept])["parallax"], json!(0.175));
}
