use super::*;

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
