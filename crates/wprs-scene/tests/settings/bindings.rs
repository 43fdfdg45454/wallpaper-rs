use super::*;

#[test]
fn masks_and_particle_textures_follow_settings() {
    let props = props(json!({ "look": { "type": "combo", "default": "a", "options": ["a", "b"] } }));
    let mapped = json!({ "prop": "look", "map": { "a": "one", "b": "two" } });
    let scene = |two: &str| {
        json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] },
            "assets": { "one": { "path": "one.png" }, two: { "path": "two.png" } },
            "layers": [{ "id": "veil", "type": "solid", "color": "#000000", "size": [64, 32], "mask": { "texture": mapped } }],
            "particles": { "dust": { "capacity": 10, "emitters": [{ "shape": { "point": [32, 16] }, "rate": 5 }],
                "init": { "life": [1, 2] }, "render": { "texture": mapped } } } })
        .to_string()
    };
    assert!(Scene::load(&scene("other"), &props).is_err());
    let loaded = std::sync::Arc::new(Scene::load(&scene("two"), &props).unwrap());
    let storm = values(json!({ "look": "b" }));
    let sprite = loaded.particles["dust"].render.texture.as_ref().unwrap();
    assert_eq!(sprite.settled(Some(&effective(&props, &[&storm]))), Some("two".to_string()));
    let mut runtime = Runtime::new(loaded, effective(&props, &[]));
    let mask = |rt: &Runtime| {
        rt.frame([0.0, 0.0, 64.0, 32.0], Quality::Q3).draws[0].mask_texture.as_deref().map(String::from)
    };
    assert_eq!(mask(&runtime), Some("one".into()));
    runtime.set_props(effective(&props, &[&storm]));
    assert_eq!(mask(&runtime), Some("two".into()));
}

#[test]
fn renaming_an_id_keeps_texts_numbers_literals_and_longer_ids() {
    let text = "mood == 'mood' && moody > 1.5 || !(mood != \"a\")";
    assert_eq!(rename(text, "mood", "feel"), "feel == 'mood' && moody > 1.5 || !(feel != \"a\")");
    assert_eq!(rename("true && x2 == 2", "true", "yes"), "true && x2 == 2");
    assert!(Expr::parse(&rename(text, "mood", "feel")).is_ok());
}

#[test]
fn problems_name_the_field_to_fix_and_groups_do_not_nest() {
    let mut props = sample();
    props.get_mut("glow").unwrap().visible_if = Condition::Expr("gone == 1".into());
    let refused = check_presets(&props, &[]).unwrap_err();
    assert_eq!(refused, ("properties.glow.visible_if".into(), "depends on the unknown property gone".into()));
    let mut props = sample();
    let inner: Property = serde_json::from_value(json!({ "type": "group", "group": "more" })).unwrap();
    props.insert("inner".into(), inner);
    assert_eq!(check_presets(&props, &[]).unwrap_err().0, "properties.inner.group");
    props.get_mut("inner").unwrap().group = None;
    props.get_mut("speed").unwrap().group = Some("inner".into());
    assert_eq!(check_presets(&props, &[]), Ok(()));
    let preset = vec![Preset { id: "p".into(), label: None, values: values(json!({ "mood": 7 })) }];
    assert_eq!(check_presets(&props, &preset).unwrap_err().0, "presets.0.values.mood");
    let every = property_schema().to_string();
    assert!(every.contains(r#""x-range":[10,86400],"x-unit":"s""#), "{every}");
}

#[test]
fn bindings_follow_values_and_triggers_follow_buttons() {
    let props = sample();
    let scene = |prop: &str, trigger: &str| {
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "assets": { "sky": { "path": "sky.png" } },
            "layers": [{ "id": "a", "type": "solid", "color": "#000000", "opacity": { "prop": prop } }],
            "timelines": [{ "id": "t", "duration": 1, "trigger": trigger }] });
        Scene::from_json(&text.to_string()).unwrap().validate(&props)
    };
    assert_eq!(scene("speed", "prop:again"), []);
    assert_eq!(scene("speed", "prop:mood"), []);
    let valueless = |path: &str, id: &str| Issue::Valueless { path: path.into(), id: id.into() };
    assert_eq!(
        scene("again", "prop:more"),
        [valueless("/layers/0/opacity", "again"), valueless("/timelines/0/trigger", "more")]
    );
    assert_eq!(scene("lines", "start"), [valueless("/layers/0/opacity", "lines")]);
    let labeled: Property = serde_json::from_value(
        json!({ "type": "combo", "default": "a", "options": [{ "value": "a", "label": "" }, { "value": "b", "label": "Bee" }] }),
    )
    .unwrap();
    let only = BTreeMap::from([("pick".to_string(), labeled)]);
    let options = &form(&only, &[], &effective(&only, &[]), &[]).0[0].options;
    assert_eq!((options[0].label.as_str(), options[1].label.as_str()), ("a", "Bee"));
}
