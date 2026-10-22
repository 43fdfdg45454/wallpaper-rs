use std::collections::BTreeMap;
use wprs_scene::*;

const SAMPLE: &str = include_str!("sample.json");

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("props.json")).unwrap()
}

fn sample() -> Scene {
    Scene::from_json(SAMPLE).unwrap()
}

fn issues(edit: impl FnOnce(&mut Scene)) -> Vec<Issue> {
    let mut scene = sample();
    edit(&mut scene);
    scene.validate(&props())
}

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
    assert_eq!(
        issues(|s| s.layers[0].opacity = Value::Fixed(2.0)),
        [range("/layers/0/opacity", 2.0, 0.0, 1.0)]
    );
    assert_eq!(
        issues(|s| s.layers[1].kind = Kind::Video { video: "gone".into(), size: None }),
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
        let found = issues(|s| s.layers[0].effects[0].mask = Some(mask));
        assert_eq!(found, [Issue::EffectMask("/layers/0/effects/0/mask".into())]);
    }
    let deep = (0..9).fold(Layer::new("leaf", Kind::Adjustment {}), |inner, i| {
        Layer::new(format!("g{i}"), Kind::Group { layers: vec![inner], clip: false })
    });
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
            ..Layer::new(format!("l{i}"), Kind::Adjustment {})
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
    let clicked = SAMPLE.replace("\"autoplay\": true", "\"trigger\": \"click:sky\"");
    assert_eq!(
        Scene::from_json(&clicked).unwrap().validate(&props()),
        [unknown("/timelines/0/trigger", "trigger", "click:sky")]
    );
}

fn compile(source: &str) -> Result<naga::Module, String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let mut validator = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), Default::default());
    validator.validate(&module).map_err(|e| e.emit_to_string(source))?;
    Ok(module)
}

#[test]
fn every_built_in_effect_matches_its_shader() {
    assert!(compile(&format!("{PRELUDE}\nfn broken(")).is_err());
    let base = compile(PRELUDE).unwrap();
    let bindings = |m: &naga::Module| m.global_variables.iter().filter(|(_, g)| g.binding.is_some()).count();
    let ids: Vec<&str> = builtin().iter().map(|d| d.id.as_str()).collect();
    assert!(ids.is_sorted() && ["blur", "color_adjust", "waves", "bloom"].iter().all(|id| ids.contains(id)));
    for d in builtin() {
        assert_eq!(d.problems(), Vec::<String>::new());
        let module = compile(&effect_source(&[d])).unwrap_or_else(|e| panic!("{}: {e}", d.id));
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        for pass in &d.passes {
            assert!(entries.contains(&pass.entry.as_str()), "{} has no entry {}", d.id, pass.entry);
        }
        assert_eq!(bindings(&module), bindings(&base), "{} declares its own bindings", d.id);
    }
    let fused: Vec<&Descriptor> = builtin().iter().filter(|d| d.fuse.is_some()).collect();
    let chain = compile(&effect_source(&fused)).unwrap_or_else(|e| panic!("fused effects clash: {e}"));
    let entries: Vec<&str> =
        base.entry_points.iter().chain(&chain.entry_points).map(|e| e.name.as_str()).collect();
    assert!([CHAIN_ENTRY, "fs_mask_mix", "vs_main"].iter().all(|e| entries.contains(e)), "{entries:?}");
    let blur = descriptor("blur").unwrap();
    let packed = blur.pack(|id| (id == "radius").then_some(Param::Number(300.0)));
    assert_eq!((packed[0], packed[1]), ([128.0, 0.0, 0.0, 0.0], [0.0; 4]));
    let waves = descriptor("waves").unwrap();
    assert_eq!(
        waves.pack(|_| None)[..4],
        [[8.0, 0.0, 0.0, 0.0], [4.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.0; 4]]
    );
    assert_eq!(
        [waves.class(), blur.class(), descriptor("bloom").unwrap().class()],
        [Class::Light, Class::Medium, Class::Medium]
    );
    assert!(waves.animates() && !blur.animates());
}

#[test]
fn built_in_effects_compile_and_report_their_cost() {
    use Class::{Light, Medium};
    let expected = [
        ("blur_motion", Medium, 2.0),
        ("blur_radial", Medium, 4.0),
        ("tilt_shift", Medium, 2.25),
        ("god_rays", Medium, 3.0),
        ("shine", Light, 1.0),
        ("color_lut", Light, 1.0),
        ("posterize", Light, 1.0),
        ("chroma_key", Light, 1.0),
        ("vignette", Light, 1.0),
        ("chromatic_aberration", Light, 1.0),
        ("film_grain", Light, 1.0),
        ("crt", Light, 1.0),
        ("pixelate", Light, 1.0),
        ("glitch", Light, 1.0),
        ("edges", Medium, 2.5),
    ];
    for (id, class, units) in expected {
        let d = descriptor(id).unwrap_or_else(|| panic!("{id} is not built in"));
        assert_eq!(d.problems(), Vec::<String>::new());
        compile(&effect_source(&[d])).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!((d.class(), d.units()), (class, units), "{id}");
        assert!(d.uses.is_empty(), "{id} wakes the scene by itself");
    }
    let depth = descriptor("depth_parallax").unwrap();
    compile(&effect_source(&[depth])).unwrap_or_else(|e| panic!("depth_parallax: {e}"));
    assert_eq!((depth.problems(), depth.class(), depth.units()), (Vec::<String>::new(), Medium, 3.0));
    let gates: Vec<Use> = depth.params.iter().filter_map(|p| p.uses).collect();
    assert_eq!((gates, depth.reach.as_deref()), (vec![Use::Cursor, Use::Time], Some("strength")));
    let steps = [Quality::Q1, Quality::Q2, Quality::Q3].map(|q| depth.setting(q, "steps", None));
    assert_eq!(steps, [Some(Param::Number(8.0)), Some(Param::Number(12.0)), None]);
    let samples = |id: &str, q| descriptor(id).and_then(|d| d.tuned(q, "samples")).cloned();
    assert_eq!(samples("god_rays", Quality::Q1), Some(Param::Number(8.0)));
    assert_eq!(samples("blur_motion", Quality::Q2), None);
    let fused = ["shine", "posterize", "chroma_key", "vignette", "film_grain", "pixelate"].map(descriptor);
    let fused: Vec<&Descriptor> = fused.into_iter().flatten().collect();
    assert!(fused.len() == 6 && fused.iter().all(|d| d.fuse.is_some()));
    compile(&effect_source(&fused)).unwrap_or_else(|e| panic!("fused chain: {e}"));
}
