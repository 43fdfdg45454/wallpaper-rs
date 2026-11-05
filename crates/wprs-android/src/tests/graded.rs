use super::*;

fn set(lock: bool, filter: Value) -> Value {
    ask(json!({ "op": "set_filter", "lock": lock, "filter": filter }))
}

pub(super) fn each_screen_has_its_own_filter() {
    let home = ask(json!({ "op": "filter", "lock": false }));
    assert_eq!((&home["own"], &home["filter"]["look"]), (&json!(false), &json!("none")), "{home}");
    assert_eq!(home["looks"].as_array().map(Vec::len), Some(16));
    assert_eq!(
        home["fields"][5],
        json!({ "key": "hue", "min": -180.0, "max": 180.0, "decimals": 0, "value": 0.0 })
    );
    assert!(settings::look(false).grade.is_none() && settings::look(true).grade.is_none());
    let warm = set(false, json!({ "look": "warm", "exposure": 0.5 }));
    assert_eq!((&warm["own"], &warm["filter"]["look"]), (&json!(true), &json!("warm")));
    assert!(settings::look(false).grade.is_some() && settings::look(true).grade.is_some());
    let lock = ask(json!({ "op": "filter", "lock": true }));
    assert_eq!((&lock["own"], &lock["filter"]["look"]), (&json!(false), &json!("warm")));
    assert_eq!(set(true, json!({}))["own"], json!(true));
    assert!(settings::look(false).grade.is_some() && settings::look(true).grade.is_none());
    assert_eq!(set(false, json!({ "look": "mono" }))["filter"]["look"], json!("mono"));
    assert!(
        settings::look(true).grade.is_none(),
        "the home screen changed the own filter of the lock screen"
    );
    assert_eq!(set(true, Value::Null)["filter"]["look"], json!("mono"));
    assert!(settings::look(true).grade.is_some());
    for bad in [json!({ "exposure": 9 }), json!({ "hue": -200 }), json!({ "strength": -0.1 })] {
        assert_eq!(set(false, bad)["id"], json!("android.filter_range"));
    }
    for bad in [json!({ "look": "neon" }), json!({ "glow": 1 }), json!("warm"), json!({ "contrast": "high" })]
    {
        assert_eq!(set(true, bad)["id"], json!("android.filter_value"));
    }
    assert_eq!(set(false, json!({}))["own"], json!(false));
    assert!(settings::look(false).grade.is_none() && settings::look(true).grade.is_none());
}
