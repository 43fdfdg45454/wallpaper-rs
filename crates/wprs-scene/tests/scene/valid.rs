use super::*;

#[test]
fn sample_roundtrips_and_is_valid() {
    let scene = Scene::load(SAMPLE, &props()).unwrap();
    assert_eq!(Scene::from_json(&scene.to_json().unwrap()).unwrap(), scene);
    let ids: Vec<&str> = scene.layers().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["background", "sea", "sparks", "clock", "badge", "disc", "ring", "bars", "post"]);
    assert_eq!(scene.canvas.background, Color::parse("#0b1020").unwrap());
    assert!(matches!(&scene.layer("bars").unwrap().kind, Kind::Spectrum(s) if s.bands == 32 && s.gap == 0.2));
    let paths: Vec<&str> = scene.paths().into_iter().collect();
    assert_eq!(paths.len(), 7);
    assert!(paths.contains(&"assets/sea.webm") && paths.contains(&"scripts/scene.js"));
    let clock = scene.layer("clock").unwrap();
    assert_eq!(
        (clock.anchor, clock.blend, clock.transform.origin),
        (Some(Anchor::BottomRight), Blend::Normal, None)
    );
}

#[test]
fn unknown_fields_and_values_are_rejected() {
    let edits = [
        ("\"opacity\": 0.6", "\"opacitty\": 0.6"),
        ("\"bands\": 32", "\"bands\": 32, \"glow\": 1"),
        ("\"effect\": \"blur\"", "\"effect\": \"blur\", \"strength\": 1"),
        ("\"type\": \"adjustment\"", "\"type\": \"adjust\""),
        ("\"loop\": \"repeat\"", "\"loop\": \"forever\""),
        ("\"out_cubic\"", "\"out_cubical\""),
        (
            "\"scale\": 0.8, \"offset\": 0.2, \"clamp\"",
            "\"scale\": 0.8, \"offset\": 0.2, \"jitter\": 1, \"clamp\"",
        ),
        ("\"fill\": \"#ff8800\"", "\"fill\": \"#ff88\""),
        ("\"capacity\": 4000", "\"capacity\": 4000, \"gravity\": 1"),
        ("\"canvas\": {", "\"canvass\": {"),
    ];
    for (from, to) in edits {
        assert!(SAMPLE.contains(from), "{from}");
        assert!(Scene::from_json(&SAMPLE.replacen(from, to, 1)).is_err(), "{to}");
    }
}

#[test]
fn validation_names_the_broken_field() {
    let range =
        |path: &str, value: f32, min: f32, max: f32| Issue::Range { path: path.into(), value, min, max };
    let unknown = |path: &str, what, id: &str| Issue::Unknown { path: path.into(), what, id: id.into() };
    assert_eq!(issues(|s| s.schema = "other/2".into()), [Issue::Schema("other/2".into())]);
    assert_eq!(issues(|s| s.canvas.size = [0, 1080]), [range("/canvas/size/0", 0.0, 1.0, 16384.0)]);
    assert_eq!(issues(|s| s.layers[1].id = "Sea".into()), [Issue::Id("/layers/1/id".into())]);
    assert_eq!(issues(|s| s.layers[2].id = "sea".into()), [Issue::DuplicateId("/layers/2/id".into())]);
    let nested = issues(|s| {
        let Kind::Group { layers, .. } = &mut s.layers[4].kind else { unreachable!() };
        layers[1].id = "Ring".into();
    });
    assert_eq!(nested, [Issue::Id("/layers/4/layers/1/id".into())]);
    assert_eq!(
        issues(|s| s.layers[0].opacity = Value::Fixed(2.0)),
        [range("/layers/0/opacity", 2.0, 0.0, 1.0)]
    );
    assert_eq!(
        issues(|s| s.layers[1].kind =
            Kind::Video { video: Value::Fixed("gone".into()), size: None, color: None, playback: None }),
        [unknown("/layers/1/video", "asset", "gone")]
    );
    assert_eq!(
        issues(|s| s.assets.remove("sky_dawn").map(drop).unwrap()),
        [unknown("/layers/0/image/map/dawn", "asset", "sky_dawn")]
    );
    assert_eq!(issues(|s| s.particles.clear()), [unknown("/layers/2/system", "particle system", "sparks")]);
    let renamed = SAMPLE
        .replace("{ \"prop\": \"depth\", \"scale\": 10 }", "{ \"prop\": \"deep\", \"audio\": \"bass\" }");
    let found = Scene::from_json(&renamed).unwrap().validate(&props());
    let path = "/layers/0/effects/1/params/radius";
    assert_eq!(found, [Issue::Source(path.into()), unknown(path, "property", "deep")]);
    let found = issues(|s| {
        s.layers[0].effects[0]
            .params
            .insert("sigma".into(), Value::Fixed(Param::Number(1.0)))
            .map_or((), drop)
    });
    assert_eq!(found, [unknown("/layers/0/effects/0/params/sigma", "parameter", "sigma")]);
    let found = issues(|s| {
        drop(s.layers[0].effects[0].params.insert("amplitude".into(), Value::Fixed(Param::Number(900.0))))
    });
    assert_eq!(found, [range("/layers/0/effects/0/params/amplitude", 900.0, 0.0, 256.0)]);
    let found = issues(|s| s.layers[0].effects[1].effect = "sparkle".into());
    assert_eq!(found, [unknown("/layers/0/effects/1/effect", "effect", "sparkle")]);
    let looped = issues(|s| {
        let Kind::Group { layers, .. } = &mut s.layers[4].kind else { unreachable!() };
        layers[0].mask = Some(Mask { layer: Some("ring".into()), ..Mask::default() });
    });
    assert_eq!(looped, [Issue::Cycle("/layers/disc".into()), Issue::Cycle("/layers/ring".into())]);
    for mask in [
        Mask { layer: Some("disc".into()), ..Mask::default() },
        Mask { shape: Some(Shape::Ellipse { size: [4.0, 4.0] }), ..Mask::default() },
    ] {
        assert!(issues(|s| s.layers[0].effects[0].mask = Some(mask)).is_empty());
    }
    let own = issues(|s| {
        let layer = Some(s.layers[0].id.clone());
        s.layers[0].effects[0].mask = Some(Mask { layer, ..Mask::default() });
    });
    assert_eq!(own, [Issue::Circle("/layers/0/effects".into())]);
    let deep = (0..9).fold(
        Layer::new("leaf", Kind::Adjustment { size: None, redraw: false, rgb: false }),
        |inner, i| {
            Layer::new(
                format!("g{i}"),
                Kind::Group { layers: vec![inner], clip: false, color: None, space: None },
            )
        },
    );
    assert_eq!(issues(|s| s.layers = vec![deep]), [Issue::Depth("/layers/0".repeat(9))]);
    let unsorted = SAMPLE.replace("[[0, 0], [10, 360]]", "[[10, 360], [0, 0]]");
    let found = Scene::from_json(&unsorted).unwrap().validate(&props());
    assert_eq!(found, [Issue::Unsorted("/layers/4/transform/rotation/keys".into())]);
    let clocked = SAMPLE.replace("\"opacity\": 0.6", "\"opacity\": { \"clock\": \"HH\" }");
    assert_eq!(
        Scene::from_json(&clocked).unwrap().validate(&props()),
        [Issue::Type("/layers/1/opacity".into())]
    );
    let huge = SAMPLE
        .replace("\"scale\": 0.8, \"offset\": 0.2, \"clamp\"", "\"scale\": 1e39, \"offset\": 0.2, \"clamp\"");
    let found = Scene::from_json(&huge).unwrap().validate(&props());
    let infinite = |i: &Issue| matches!(i, Issue::Range { path, value, .. } if path.ends_with("/opacity/scale") && value.is_infinite());
    assert!(found.len() == 1 && infinite(&found[0]), "{found:?}");
    let curve = SAMPLE.replace("\"out_cubic\"", "[0.2, 0, 1.5, 1]");
    let found = Scene::from_json(&curve).unwrap().validate(&props());
    assert_eq!(found, [range("/layers/3/opacity/keys/0/2/2", 1.5, 0.0, 1.0)]);
    let chain: Vec<Layer> = (0..2000)
        .map(|i| Layer {
            mask: Some(Mask { layer: Some(format!("l{}", i + 1)), ..Mask::default() }),
            ..Layer::new(format!("l{i}"), Kind::Adjustment { size: None, redraw: false, rgb: false })
        })
        .collect();
    let started = std::time::Instant::now();
    assert_eq!(
        issues(|s| s.layers = chain),
        [Issue::TooMany { path: "/layers".into(), count: 2000, max: 256 }]
    );
    assert!(started.elapsed().as_secs() < 2);
    let triggered = SAMPLE.replace("\"autoplay\": true", "\"trigger\": \"prop:nothing\"");
    assert_eq!(
        Scene::from_json(&triggered).unwrap().validate(&props()),
        [unknown("/timelines/0/trigger", "property", "nothing")]
    );
    let tapped = SAMPLE.replace("\"autoplay\": true", "\"trigger\": \"tap:sky\"");
    assert_eq!(
        Scene::from_json(&tapped).unwrap().validate(&props()),
        [unknown("/timelines/0/trigger", "trigger", "tap:sky")]
    );
    let clicked = SAMPLE.replace("\"autoplay\": true", "\"trigger\": \"click:sky\"");
    assert_eq!(
        Scene::from_json(&clicked).unwrap().validate(&props()),
        [unknown("/timelines/0/trigger", "layer that takes clicks", "sky")]
    );
}
