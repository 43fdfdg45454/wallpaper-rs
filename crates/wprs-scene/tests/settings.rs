use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_scene::*;

fn props(value: Value) -> BTreeMap<String, Property> {
    serde_json::from_value(value).unwrap()
}

fn values(value: Value) -> Values {
    serde_json::from_value(value).unwrap()
}

fn sample() -> BTreeMap<String, Property> {
    props(json!({
        "mood": { "type": "combo", "default": 1, "icon": "weather-storm",
                  "options": [{ "value": 1, "label": "Calm", "icon": "weather-clear" }, { "value": 2, "label": "Storm", "icon": "x-nope" }] },
        "speed": { "type": "slider", "min": 0.05, "max": 4, "decimals": 2, "editable": true, "log": true, "default": 1 },
        "glow": { "type": "bool", "default": true, "visible_if": "mood == 2 || speed > 1.5", "group": "more" },
        "more": { "type": "group", "collapsed": true, "order": 2.5 },
        "lines": { "type": "divider", "label": "Lines", "order": 3 },
        "again": { "type": "button", "enabled_if": "!glow && mood != 1", "order": 4 },
        "key": { "type": "text", "secret": true },
        "photo": { "type": "file", "accept": ["image", "video"], "default_asset": "sky" },
        "photos": { "type": "folder", "accept": ["image"], "pick": "sequence", "every": 30 }
    }))
}

fn holds(text: &str, given: Value) -> Result<bool, String> {
    Ok(Expr::parse(text)?.eval(&values(given)) == json!(true))
}

#[test]
fn expressions_compare_combine_and_refuse_what_is_not_a_condition() {
    let given = json!({ "a": 2, "b": "x", "c": true });
    let cases = [
        ("a == 2", true),
        ("a != 2 || b == 'x'", true),
        ("!(a > 1) && c", false),
        ("a >= 2 && a <= 2.0 && !(b == \"y\")", true),
        ("(c)", true),
        ("a < -1", false),
    ];
    for (text, want) in cases {
        assert_eq!(holds(text, given.clone()), Ok(want), "{text}");
    }
    let bad = ["a ==", "(a == 2", "a = 2", "a == 'x", "a == 2 b", "a % 2", &"(".repeat(9), &"a".repeat(257)];
    for text in bad {
        assert!(Expr::parse(text).is_err(), "{text}");
    }
    assert_eq!(Expr::parse("a > 1 && (b == 'x' || !c)").unwrap().ids(), ["a", "b", "c"]);
    let props = sample();
    let check = |cond: &str| {
        let mut props = props.clone();
        props.get_mut("glow").unwrap().visible_if = Condition::Expr(cond.into());
        check_presets(&props, &[])
    };
    assert_eq!(check("mood == 2 && speed < 3"), Ok(()));
    for refused in ["mood == 'storm'", "mood", "!speed", "nothing == 1", "glow", "lines == 1", "key < 'a'"] {
        assert!(check(refused).is_err(), "{refused}");
    }
}

#[test]
fn conditions_that_depend_on_each_other_are_refused() {
    let mut props = sample();
    props.get_mut("mood").unwrap().enabled_if = Condition::Expr("glow".into());
    let (field, refused) = check_presets(&props, &[]).unwrap_err();
    assert!(refused.contains("depend on themselves") && field.starts_with("properties."), "{refused}");
}

#[test]
fn newer_controls_check_their_fields_and_values() {
    let props = sample();
    assert!(props.values().all(|p| p.check().is_ok()));
    assert_eq!(check_presets(&props, &[]), Ok(()));
    let value = |id: &str, v: Value| props[id].value(&v);
    assert_eq!(value("speed", json!(1.23456)), Ok(json!(1.23)));
    assert_eq!(value("speed", json!(9)), Ok(json!(4.0)));
    assert_eq!((value("mood", json!(2.0)), value("mood", json!("2")).is_err()), (Ok(json!(2)), true));
    for id in ["again", "more", "lines"] {
        assert!(value(id, json!(true)).is_err() && !props[id].holds_value());
    }
    assert!(props["key"].personal() && props["photos"].personal() && !props["mood"].personal());
    assert!(props["photo"].accepts("video") && !props["photos"].accepts("video"));
    assert_eq!(effective(&props, &[])["photo"], json!("sky"));
    let broken = |p: Value| serde_json::from_value::<Property>(p).unwrap().check().is_err();
    assert!(broken(json!({ "type": "slider", "min": 0, "max": 1, "default": 0.5, "log": true })));
    assert!(broken(json!({ "type": "slider", "min": 0, "max": 1, "default": 0.5, "decimals": 7 })));
    assert!(broken(json!({ "type": "combo", "default": 1, "options": [1, "two"] })));
    assert!(broken(json!({ "type": "combo", "default": true, "options": [true, true] })));
    assert!(broken(json!({ "type": "text", "secret": true, "default": "hunter2" })));
    assert!(broken(json!({ "type": "folder", "every": 5 })));
    assert!(broken(json!({ "type": "bool", "default": true, "icon": "Not An Icon" })));
    let presets = |given: Value| vec![Preset { id: "p".into(), label: None, values: values(given) }];
    assert_eq!(check_presets(&props, &presets(json!({ "mood": 2, "speed": 0.5 }))), Ok(()));
    for personal in [
        json!({ "key": "x" }),
        json!({ "photos": "/p" }),
        json!({ "photo": "/a.png" }),
        json!({ "again": true }),
    ] {
        assert!(check_presets(&props, &presets(personal.clone())).is_err(), "{personal}");
    }
}

#[test]
fn forms_show_state_icons_and_hide_secrets() {
    let props = sample();
    let chosen = effective(&props, &[&values(json!({ "mood": 2, "photo": "/pictures/a.png" }))]);
    let (fields, _) = form(&props, &[], &chosen, &[]);
    let find = |key: &str| fields.iter().find(|f| f.key == key).unwrap();
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, ["key", "mood", "photo", "photos", "speed", "more", "glow", "lines", "again"]);
    assert!(find("glow").visible && find("more").collapsed && find("glow").group == "more");
    assert!(!find("again").enabled, "glow is on, so the button is off");
    assert_eq!(
        (find("mood").icon.as_str(), find("speed").decimals, find("speed").log),
        ("weather-storm", Some(2), true)
    );
    let icons: Vec<&str> = find("mood").options.iter().map(|o| o.icon.as_str()).collect();
    assert_eq!((find("mood").options[1].value.clone(), icons), (json!(2), vec!["weather-clear", ""]));
    assert!(find("key").secret && find("key").value.is_null());
    let accept = vec!["image".to_string(), "video".into()];
    assert_eq!(
        (find("photo").value.clone(), find("photo").accept.clone()),
        (json!("/pictures/a.png"), accept)
    );
    let unset = form(&props, &[], &effective(&props, &[]), &[]).0;
    assert!(unset.iter().find(|f| f.key == "photo").unwrap().value.is_null());
}

#[test]
fn content_gets_tokens_for_chosen_files_and_labels_for_options() {
    let props = sample();
    let chosen = effective(&props, &[&values(json!({ "photos": "/pictures" }))]);
    let shown = exposed(&props, &chosen);
    assert_eq!(
        (&shown["photos"], &shown["photo"], &shown["mood"]),
        (&json!("wpak-user://photos"), &json!(null), &json!(1))
    );
    assert_eq!(labels(&props, &[])["mood"], json!({ "1": "Calm", "2": "Storm" }));
}

#[test]
fn features_name_what_older_versions_cannot_read() {
    let props = sample();
    let used = |id: &str| props[id].features();
    assert_eq!(
        (used("speed"), used("photos"), used("photo")),
        (vec!["properties2"], vec!["folder"], vec!["properties2", "user_videos"])
    );
    let plain: BTreeMap<String, Property> = serde_json::from_str(include_str!("props.json")).unwrap();
    assert!(plain.values().all(|p| p.features().is_empty()));
    let raw: BTreeMap<String, Value> = serde_json::from_str(include_str!("props.json")).unwrap();
    assert!(raw.values().all(legacy));
    assert!(props.values().map(|p| serde_json::to_value(p).unwrap()).filter(legacy).count() < props.len());
    let round = |p: &Property| serde_json::from_value::<Property>(serde_json::to_value(p).unwrap()).unwrap();
    assert!(props.values().all(|p| round(p) == *p));
    assert_eq!(serde_json::to_value(&props["more"]).unwrap()["order"], json!(2.5));
    assert_eq!(serde_json::to_value(&props["lines"]).unwrap()["order"], json!(3));
}

#[test]
fn property_definitions_have_a_json_schema() {
    let schema = serde_json::to_value(schemars::schema_for!(Property)).unwrap().to_string();
    for kind in
        ["slider", "combo", "folder", "button", "divider", "visible_if", "enabled_if", "default_asset"]
    {
        assert!(schema.contains(kind), "{kind}");
    }
}

#[test]
fn sliders_round_dividers_hide_their_ids_and_groups_lend_their_conditions() {
    let props = props(json!({
        "level": { "type": "slider", "min": 0, "max": 1, "default": 0.5 },
        "rule": { "type": "divider" },
        "tools": { "type": "group", "enabled_if": "level > 0.6" },
        "go": { "type": "button", "group": "tools" }
    }));
    assert_eq!(props["level"].value(&json!(0.33333)), Ok(json!(0.33)));
    let low = effective(&props, &[]);
    let (fields, _) = form(&props, &[], &low, &[]);
    let find = |key: &str| fields.iter().find(|f| f.key == key).unwrap();
    assert_eq!((find("level").decimals, find("level").step), (Some(2), Some(0.01)));
    assert_eq!(find("rule").label, "");
    assert!(!find("go").enabled && !props["go"].met(&props, &low, |p| &p.enabled_if));
    let high = effective(&props, &[&values(json!({ "level": 0.8 }))]);
    assert!(props["go"].met(&props, &high, |p| &p.enabled_if));
}

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
