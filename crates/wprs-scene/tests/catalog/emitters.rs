use super::*;

fn spot(from: [f32; 2], strength: f32) -> EffectEmitter {
    EffectEmitter { from, to: None, angle: -90.0, strength }
}

#[test]
fn emitters_pack_three_texels_each_and_name_their_picture_by_content() {
    let line = EffectEmitter { from: [0.1, 1.0], to: Some([0.9, 1.0]), angle: 45.0, strength: 8.0 };
    let list = [spot([0.5, 0.9], 4.0), line];
    let texels = emitter_texels(&list);
    assert_eq!(texels.len(), 2 * EMITTER_TEXELS);
    let word = |t: [u8; 4], k: usize| f32::from(u16::from_be_bytes([t[k], t[k + 1]])) / 65535.0;
    assert!((word(texels[0], 0) * 6.0 - 2.0 - 0.5).abs() < 1e-4);
    assert_eq!(texels[0], texels[1], "a point ends where it starts");
    assert!((word(texels[4], 0) * 6.0 - 2.0 - 0.9).abs() < 1e-4);
    assert!((word(texels[5], 0) * 720.0 - 360.0 - 45.0).abs() < 0.02);
    assert!((word(texels[5], 2) * 64.0 - 8.0).abs() < 1e-3);
    assert_ne!(emitter_key(&list), emitter_key(&list[..1]));
    assert_eq!(emitter_key(&list), emitter_key(&[list[0], list[1]]));
    let many = vec![spot([0.5; 2], 1.0); MAX_EFFECT_EMITTERS + 4];
    assert_eq!(emitter_texels(&many).len(), MAX_EFFECT_EMITTERS * EMITTER_TEXELS);
}

#[test]
fn emitters_outside_their_ranges_do_not_fit() {
    assert!(spot([0.5, 0.5], 64.0).fits());
    for bad in
        [spot([f32::NAN, 0.5], 1.0), spot([5.0, 0.5], 1.0), spot([0.5, 0.5], 65.0), spot([0.5, 0.5], -1.0)]
    {
        assert!(!bad.fits(), "{bad:?}");
    }
    let turned = EffectEmitter { angle: 400.0, ..spot([0.5; 2], 1.0) };
    let far = EffectEmitter { to: Some([0.5, -3.0]), ..spot([0.5; 2], 1.0) };
    assert!(!turned.fits() && !far.fits());
}

#[test]
fn fluid_takes_any_number_of_point_and_line_emitters_up_to_the_cap() {
    let list: Vec<Json> = (0..12).map(|i| json!({ "from": [i as f32 / 12.0, 0.9], "strength": 4 })).collect();
    let mut list = list;
    list.push(json!({ "from": [0.1, 1], "to": [0.9, 1], "angle": -80, "strength": 6 }));
    assert_eq!(
        issues(effect(json!({ "effect": "fluid", "params": { "emitters": list } }))),
        Vec::<String>::new()
    );
    let crowd: Vec<Json> = (0..65).map(|_| json!({ "from": [0.5, 0.5], "strength": 1 })).collect();
    let cases = [
        json!({ "emitters": crowd }),
        json!({ "emitters": [{ "from": [0.5, 0.5], "strength": 70 }] }),
        json!({ "emitters": [{ "from": [0.5, 0.5], "angle": 400, "strength": 1 }] }),
        json!({ "emitters": { "prop": "flow" } }),
        json!({ "emitters": 3 }),
    ];
    for params in cases {
        let found = issues(effect(json!({ "effect": "fluid", "params": params.clone() })));
        assert!(found.iter().any(|f| f.contains("/params/emitters")), "{params}: {found:?}");
    }
    let unknown = Scene::from_json(
        &json!({ "schema": SCHEMA, "canvas": { "size": [8, 8] },
        "layers": [{ "id": "a", "type": "solid", "color": "#000", "effects": [{ "effect": "fluid",
        "params": { "emitters": [{ "from": [0.5, 0.5], "strength": 1, "spin": 2 }] } }] }] })
        .to_string(),
    );
    assert!(
        unknown.is_err()
            || unknown.unwrap().validate(&BTreeMap::new()).iter().any(|i| i.to_string().contains("emitters"))
    );
}

#[test]
fn emitter_lists_may_be_keyframed_or_follow_a_property_through_a_map() {
    let a = json!([{ "from": [0.2, 0.9], "angle": -90, "strength": 2 }]);
    let b = json!([{ "from": [0.6, 0.5], "angle": 0, "strength": 6 }]);
    let keyed = json!({ "emitters": { "keys": [[0, a], [2, b]] } });
    let mut doc = effect(json!({ "effect": "fluid", "params": keyed }));
    assert_eq!(issues(doc.clone()), Vec::<String>::new());
    let mapped = json!({ "emitters": { "prop": "flow", "map": { "true": a, "false": [] } } });
    doc["layers"][0]["effects"][0]["params"] = mapped;
    let props = flow();
    let found: Vec<String> = scene(doc).validate(&props).iter().map(ToString::to_string).collect();
    assert!(!found.iter().any(|f| f.contains("/params/emitters")), "{found:?}");
    let runtime = Runtime::new(
        std::sync::Arc::new(scene(effect(json!({ "effect": "fluid", "params": keyed })))),
        Default::default(),
    );
    let mut runtime = runtime;
    runtime.advance(1.0);
    let frame = runtime.frame([0.0, 0.0, 64.0, 32.0], Quality::Q3);
    let (id, list) = &frame.effects[0].emitters[0];
    assert_eq!(*id, "emitters");
    let m = list[0];
    assert!(
        (m.from[0] - 0.4).abs() < 1e-4 && (m.angle + 45.0).abs() < 1e-3 && (m.strength - 4.0).abs() < 1e-4
    );
}

#[test]
fn emitter_lists_follow_no_number_and_refuse_keys_outside_their_ranges() {
    let a = json!([{ "from": [0.2, 0.9], "strength": 2 }]);
    let cases = [
        json!({ "audio": "bass" }),
        json!({ "cursor": "x" }),
        json!({ "keys": [[0, a], [1, [{ "from": [9, 0.5], "strength": 1 }]]] }),
        json!({ "keys": [[0, a], [1, 3]] }),
        json!({ "keys": [[0, a]], "scale": 2 }),
        json!({ "prop": "flow", "map": { "true": [{ "from": [0.5, 0.5], "strength": 99 }] } }),
    ];
    for bound in cases {
        let doc = effect(json!({ "effect": "fluid", "params": { "emitters": bound.clone() } }));
        let found: Vec<String> = scene(doc).validate(&flow()).iter().map(ToString::to_string).collect();
        assert!(found.iter().any(|f| f.contains("/params/emitters")), "{bound}: {found:?}");
    }
}

#[test]
fn emitters_blend_between_keys_of_the_same_shape_and_jump_otherwise() {
    let line = EffectEmitter { from: [0.0, 1.0], to: Some([1.0, 1.0]), angle: 0.0, strength: 0.0 };
    let wide = EffectEmitter { from: [1.0, 0.0], to: Some([0.0, 0.0]), angle: 90.0, strength: 8.0 };
    let half = line.lerp(&wide, 0.5);
    assert_eq!((half.from, half.to, half.angle, half.strength), ([0.5, 0.5], Some([0.5, 0.5]), 45.0, 4.0));
    let point = spot([0.5, 0.5], 1.0);
    assert_eq!(point.lerp(&line, 0.5), point);
    assert_eq!(point.lerp(&line, 1.0), line);
}

#[test]
fn scripts_set_the_emitter_list_a_fluid_draws() {
    let doc = effect(json!({ "effect": "fluid" }));
    let mut runtime = Runtime::new(std::sync::Arc::new(scene(doc)), Default::default());
    let line = EffectEmitter { from: [0.1, 1.0], to: Some([0.9, 1.0]), angle: -80.0, strength: 3.0 };
    let set = |list: Vec<EffectEmitter>| {
        let params = [("emitters".to_string(), Param::Emitters(list))].into();
        let effects =
            [("sky".to_string(), vec![EffectControl { at: 0, params, ..Default::default() }])].into();
        ScriptOutputs { effects, ..Default::default() }
    };
    assert!(runtime.set_script(set(vec![spot([0.5, 0.9], 8.0), line])));
    let frame = runtime.frame([0.0, 0.0, 64.0, 32.0], Quality::Q3);
    let fx = &frame.effects[0];
    assert_eq!(fx.emitters[0].1, [spot([0.5, 0.9], 8.0), line]);
    let slot = fx.descriptor.params.iter().position(|p| p.id == "emitters").unwrap();
    assert_eq!(fx.slots[slot][0], 2.0, "the shader is told two emitters blow");
    drop(frame);
    let crowd = vec![spot([0.5, 0.5], 1.0); MAX_EFFECT_EMITTERS + 1];
    runtime.set_script(set(crowd));
    let frame = runtime.frame([0.0, 0.0, 64.0, 32.0], Quality::Q3);
    assert!(frame.effects[0].emitters[0].1.is_empty(), "a list over the cap is dropped");
}

fn flow() -> BTreeMap<String, Property> {
    serde_json::from_value(json!({ "flow": { "type": "bool", "label": "Flow", "default": true } })).unwrap()
}
