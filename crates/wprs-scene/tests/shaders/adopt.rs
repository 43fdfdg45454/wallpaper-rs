use super::*;

#[test]
fn scenes_adopt_their_package_effects_and_check_their_uses() {
    let mut fine =
        scene(json!([{ "effect": "ripple", "params": { "detail": 3, "taps": 8, "mode": "glow" } }]));
    assert_eq!(fine.features().into_iter().collect::<Vec<_>>(), ["custom_effects"]);
    adopt(&mut fine, &descriptor()).unwrap();
    let found: Vec<_> = fine.features().into_iter().collect();
    assert_eq!(found, ["custom_effects", "effect_options", "vertex_effects"]);
    assert!(fine.effect("ripple").is_some() && fine.effect("blur").is_some() && needs(&fine).audio);
    for (params, problem) in [
        (json!({ "detail": 9 }), "/layers/0/effects/0/params/detail is 9, expected 0..=4"),
        (json!({ "taps": 5 }), "/layers/0/effects/0/params/taps does not fit"),
        (json!({ "mode": "tint" }), "/layers/0/effects/0/params/mode does not fit"),
        (json!({ "missing": 1 }), "names the unknown parameter"),
    ] {
        let mut wrong = scene(json!([{ "effect": "ripple", "params": params }]));
        let fault = adopt(&mut wrong, &descriptor()).unwrap_err().to_string();
        assert!(fault.contains(problem), "{fault}");
    }
    let mut wide = descriptor();
    wide["options"] = json!([{ "id": "taps", "values": [1, 2, 3, 4, 5], "default": 1 },
        { "id": "tinted", "values": [false, true], "default": false }]);
    let props: BTreeMap<String, Property> = serde_json::from_value(json!({
        "q": { "type": "slider", "min": 1, "max": 8, "default": 1 }, "t": { "type": "bool", "default": false } })).unwrap();
    let mut bound = scene(
        json!([{ "effect": "ripple", "params": { "taps": { "prop": "q" }, "tinted": { "prop": "t" } } }]),
    );
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with("json") { wide.to_string() } else { WGSL.into() }.into_bytes())
    };
    let fault = bound.adopt(&props, files).unwrap_err().to_string();
    assert_eq!(
        fault,
        "invalid scene: /layers/0/effects/0/params needs 10 variants of its shader, at most 8 are allowed"
    );
    let mut waiting = descriptor();
    waiting["options"][0]["values"] = json!([1, 2, 3, 4, 5]);
    waiting["options"][0]["default"] = json!(1);
    let mut both = scene(json!([{ "effect": "ripple",
        "params": { "taps": { "prop": "q" }, "paint": { "prop": "t", "map": { "false": "a", "true": "a" } } } }]));
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with("json") { waiting.to_string() } else { WGSL.into() }.into_bytes())
    };
    let fault = both.adopt(&props, files).unwrap_err().to_string();
    assert!(fault.ends_with("needs 10 variants of its shader, at most 8 are allowed"), "{fault}");
    let d = packaged(|_| (), WGSL).unwrap();
    let fed: EffectUse =
        serde_json::from_value(json!({ "effect": "ripple", "params": { "paint": { "layer": "sky" } } }))
            .unwrap();
    assert_eq!((d.variant(&fed, |_| None), d.variants(&fed).unwrap()), (1 << 4, vec![1 << 4]));
    let painted: EffectUse =
        serde_json::from_value(json!({ "effect": "ripple", "params": { "paint": "a" } })).unwrap();
    assert_eq!(d.variants(&painted).unwrap(), vec![1 << 4]);
    let plain: EffectUse = serde_json::from_value(json!({ "effect": "ripple" })).unwrap();
    assert_eq!((d.variant(&plain, |_| None), d.variants(&plain).unwrap()), (0, vec![0]));
    let mut gone = scene(json!([{ "effect": "ripple" }]));
    let files = |_: &str, _: usize| Err::<Vec<u8>, _>("no such file");
    assert!(
        gone.adopt(&BTreeMap::new(), files)
            .unwrap_err()
            .to_string()
            .contains("/effects/ripple: fx/ripple.json")
    );
}

#[test]
fn package_effects_compile_once_and_share_a_shader_budget() {
    let (mut first, mut second) =
        (scene(json!([{ "effect": "ripple" }])), scene(json!([{ "effect": "ripple" }])));
    adopt(&mut first, &descriptor()).unwrap();
    adopt(&mut second, &descriptor()).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.loaded["ripple"], &second.loaded["ripple"]));
    let effects: serde_json::Map<String, Json> = (0..5)
        .map(|i| {
            (
                format!("e{i}"),
                json!({ "descriptor": format!("fx/e{i}.json"), "wgsl": format!("fx/e{i}.wgsl") }),
            )
        })
        .collect();
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "effects": effects, "layers": [] });
    let mut many = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap();
    let files = |path: &str, max: usize| {
        let id = path.trim_start_matches("fx/").split('.').next().unwrap_or_default();
        let mut d = descriptor();
        d["id"] = json!(id);
        let text =
            if path.ends_with("json") { d.to_string() } else { format!("{WGSL}{}", " ".repeat(60 << 10)) };
        Some(text.into_bytes()).filter(|b| b.len() <= max).ok_or("too large")
    };
    let fault = many.adopt(&BTreeMap::new(), files).unwrap_err().to_string();
    assert!(fault.contains("the shaders of the package pass 256 KiB"), "{fault}");
}
