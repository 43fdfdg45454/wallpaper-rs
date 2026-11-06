use super::*;

#[test]
fn templates_become_layers_in_the_order_scripts_give() {
    let mut rt = runtime(json!({}));
    let spawned = vec![
        ("one".to_string(), "bar".to_string()),
        ("two".into(), "bar".into()),
        ("back".into(), "bar".into()),
    ];
    assert!(rt.set_script(ScriptOutputs { spawned: spawned.clone(), ..ScriptOutputs::default() }));
    assert_eq!(ids(&rt), ["back", "label", "one", "one.fill", "two", "two.fill"]);
    let first = rt.spawned().next().map(|l| l as *const Layer);
    let order = Some(vec!["two".to_string(), "back".into(), "one".into()]);
    let spawned = vec![
        ("two".to_string(), "bar".to_string()),
        ("one".into(), "bar".into()),
        ("x".into(), "none".into()),
    ];
    assert!(rt.set_script(ScriptOutputs {
        spawned: spawned.clone(),
        order: order.clone(),
        ..ScriptOutputs::default()
    }));
    assert_eq!(ids(&rt), ["two", "two.fill", "back", "one", "one.fill", "label"]);
    assert_eq!(rt.spawned().nth(1).map(|l| l as *const Layer), first, "a kept layer moved in memory");
    assert!(!rt.set_script(ScriptOutputs {
        spawned,
        order,
        console: vec!["log".into()],
        ..ScriptOutputs::default()
    }));
    let many = (0..300).map(|i| (format!("b{i}"), "bar".to_string())).collect();
    rt.set_script(ScriptOutputs { spawned: many, ..ScriptOutputs::default() });
    assert_eq!(rt.spawned().count(), (MAX_LAYERS + MAX_SPAWNED - 2) / 2);
    assert!(rt.layer("b254.fill").is_some() && rt.layer("b255").is_none());
}

#[test]
fn big_templates_are_refused_and_copies_stop_at_the_layer_limit() {
    let children = |n: usize| -> Vec<serde_json::Value> {
        (0..n).map(|i| json!({ "id": format!("c{i}"), "type": "solid", "color": "#ffffff" })).collect()
    };
    let doc = |n: usize| {
        json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
            "templates": { "big": { "id": "big", "type": "group", "layers": children(n) } } })
        .to_string()
    };
    let error = Scene::load(&doc(2000), &Default::default()).unwrap_err().to_string();
    assert!(error.contains("/templates/big/layers"), "{error}");
    let scene = Scene::load(&doc(199), &Default::default()).unwrap();
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    let many = (0..256).map(|i| (format!("b{i}"), "big".to_string())).collect();
    rt.set_script(ScriptOutputs { spawned: many, ..ScriptOutputs::default() });
    assert_eq!(rt.spawned().count(), 2);
    assert_eq!(rt.spawned().map(|l| l.children().len() + 1).sum::<usize>(), 400);
}

#[test]
fn effect_controls_keep_to_what_the_effect_declares() {
    let mut rt = runtime(json!({}));
    let mut params = std::collections::BTreeMap::new();
    params.insert("radius".to_string(), Param::Number(500.0));
    params.insert("glow".to_string(), Param::Number(1.0));
    params.insert("texture".to_string(), Param::Text("x".into()));
    let soft = EffectControl { at: 0, enabled: None, params, reset: 2 };
    let off = EffectControl { at: 1, enabled: Some(false), ..EffectControl::default() };
    let stray = EffectControl { at: 9, enabled: Some(false), ..EffectControl::default() };
    let effects =
        [("back".to_string(), vec![soft, off, stray]), ("label".into(), vec![EffectControl::default()])]
            .into();
    rt.set_script(ScriptOutputs { effects, ..ScriptOutputs::default() });
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(frame.effects.len(), 1);
    let blur = &frame.effects[0];
    assert_eq!((blur.descriptor.id.as_str(), blur.slots[0][0], blur.reset), ("blur", 128.0, 2));
}

#[test]
fn templates_only_copy_what_scripts_can_draw() {
    let text = |kind: serde_json::Value| {
        json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] }, "assets": { "v": { "path": "v.webm" } },
            "templates": { "t": kind } })
        .to_string()
    };
    let video = json!({ "id": "t", "type": "video", "video": "v" });
    assert!(
        Scene::load(&text(video), &Default::default()).unwrap_err().to_string().contains("/templates/t/type")
    );
    let shape =
        json!({ "id": "t", "type": "shape", "shape": { "rect": { "size": [2, 2] } }, "fill": "#ffffff" });
    assert!(Scene::load(&text(shape), &Default::default()).is_ok());
}

#[test]
fn copies_of_templates_count_in_the_cost_of_the_scene() {
    let template =
        json!({ "id": "glow", "type": "solid", "color": "#ffffff", "effects": [{ "effect": "blur" }] });
    let mut rt = runtime(json!({ "templates": { "glow": template } }));
    let alone = rt.cost(Quality::Q3);
    let many = (0..10).map(|i| (format!("g{i}"), "glow".to_string())).collect();
    rt.set_script(ScriptOutputs { spawned: many, ..ScriptOutputs::default() });
    assert!(rt.cost(Quality::Q3) > alone + 9.0, "{} {alone}", rt.cost(Quality::Q3));
    rt.set_script(ScriptOutputs::default());
    assert_eq!(rt.cost(Quality::Q3), alone);
}

#[test]
fn bloom_a_script_turns_on_counts_in_the_cost_until_it_is_turned_off() {
    let mut rt = runtime(json!({}));
    let alone = rt.cost(Quality::Q3);
    let bloom = BloomSet { threshold: Some(0.5), strength: Some(2.0), radius: Some(8.0), tint: None };
    let stage = Some(StageSet { bloom: Some(bloom), ..StageSet::default() });
    rt.set_script(ScriptOutputs { stage, ..ScriptOutputs::default() });
    assert!(rt.cost(Quality::Q3) > alone + 1.0, "{} {alone}", rt.cost(Quality::Q3));
    rt.set_script(ScriptOutputs::default());
    assert_eq!(rt.cost(Quality::Q3), alone);
}

#[test]
fn a_script_that_may_turn_bloom_on_counts_it_in_the_cost_of_the_scene_and_its_features() {
    let scene = |uses: serde_json::Value| {
        let doc = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] },
            "scripts": [{ "path": "main.js", "uses": uses }],
            "layers": [{ "id": "fill", "type": "solid", "color": "#808080" }] });
        Scene::from_json(&doc.to_string()).unwrap()
    };
    let (lit, plain) = (scene(json!(["bloom"])), scene(json!(["camera"])));
    assert!(
        lit.cost(Quality::Q3) > plain.cost(Quality::Q3) + 1.0,
        "{} {}",
        lit.cost(Quality::Q3),
        plain.cost(Quality::Q3)
    );
    assert!(lit.features().contains("script_camera") && plain.features().contains("script_camera"));
    let rt = Runtime::new(std::sync::Arc::new(lit), Default::default());
    assert_eq!(
        rt.cost(Quality::Q3),
        plain.cost(Quality::Q3),
        "the run counts the glow only once a script asks for it"
    );
}

#[test]
fn masks_and_layer_feeds_inside_a_template_point_at_the_layers_of_each_copy() {
    let pic = json!({ "id": "pic", "type": "solid", "color": "#ffffff", "mask": { "layer": "shape" },
        "effects": [{ "effect": "tint", "mask": { "layer": "shape" } },
            { "effect": "displace", "params": { "map": { "layer": "shape" } } }] });
    let card = json!({ "id": "card", "type": "group",
        "layers": [{ "id": "shape", "type": "solid", "color": "#000000" }, pic] });
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] },
        "scripts": [{ "path": "main.js", "uses": ["layers"] }], "layers": [], "templates": { "card": card } });
    let scene = Scene::from_json(&doc.to_string()).unwrap();
    assert!(scene.validate(&Default::default()).is_empty(), "{:?}", scene.validate(&Default::default()));
    let mut rt = Runtime::new(Arc::new(scene), Default::default());
    let spawned = vec![("c1".to_string(), "card".to_string())];
    assert!(rt.set_script(ScriptOutputs { spawned, ..ScriptOutputs::default() }));
    let copy = rt.spawned().flat_map(|l| l.children().iter()).find(|l| l.id == "c1.pic").unwrap();
    assert_eq!(copy.mask.as_ref().and_then(|m| m.layer.as_deref()), Some("c1.shape"));
    assert_eq!(copy.effects[0].mask.as_ref().and_then(|m| m.layer.as_deref()), Some("c1.shape"));
    let fed = copy.effects[1].params["map"].binding().and_then(|b| b.layer.as_deref());
    assert_eq!(fed, Some("c1.shape"));
}
