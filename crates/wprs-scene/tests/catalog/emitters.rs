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
