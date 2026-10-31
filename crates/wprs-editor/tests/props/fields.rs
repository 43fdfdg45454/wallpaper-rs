use super::*;

#[test]
fn conditions_are_checked_while_they_are_typed() {
    let mut doc = sample();
    let shown = "/manifest/properties/photo/visible_if";
    assert_eq!(condition(&doc, shown, "speed > 1 && mood != 'calm'"), None);
    assert_eq!(condition(&doc, shown, r#"{"mood": "storm"}"#), None);
    assert_eq!(condition(&doc, shown, ""), None);
    assert!(condition(&doc, shown, "mood == 2").unwrap().contains("compares a text with a number"));
    assert!(condition(&doc, shown, "rain").unwrap().contains("unknown property rain"));
    assert!(condition(&doc, shown, "speed >").unwrap().contains("ends too early"));
    assert!(condition(&doc, shown, "go").unwrap().contains("no value"));
    assert!(condition(&doc, shown, "photo == ''").unwrap().contains("unknown property photo"));
    let looped = "/manifest/properties/mood/visible_if";
    assert!(condition(&doc, looped, "speed > 2").unwrap().contains("depend on themselves"));
    assert!(
        refused(&doc, json!({"condition": {"id": "photo", "text": "speed >"}})).contains("photo: visible_if")
    );
    run(&mut doc, json!({"condition": {"id": "photo", "text": " speed >= 2 "}}));
    run(&mut doc, json!({"condition": {"id": "photo", "enabled": true, "text": r#"{"mood": "storm"}"#}}));
    assert_eq!(doc["manifest"]["properties"]["photo"]["visible_if"], "speed >= 2");
    assert_eq!(doc["manifest"]["properties"]["photo"]["enabled_if"], json!({"mood": "storm"}));
    run(&mut doc, json!({"condition": {"id": "photo", "text": ""}}));
    assert!(doc["manifest"]["properties"]["photo"].get("visible_if").is_none());
    assert_eq!(field(&doc, "/manifest/properties/photo", "enabled_if").widget, "condition");
}

#[test]
fn the_inspector_knows_every_field_of_every_kind() {
    let mut doc = sample();
    let speed = fields(&doc, "/manifest/properties/speed");
    let keys: Vec<&str> = speed.iter().map(|f| f.key.as_str()).take(7).collect();
    assert_eq!(keys, ["label", "help", "group", "order", "icon", "visible_if", "enabled_if"]);
    let default = field(&doc, "/manifest/properties/speed", "default");
    assert_eq!(
        (default.widget.as_str(), default.range.clone(), default.unit.as_str()),
        ("number", json!([0.5, 4]), "x")
    );
    assert_eq!(field(&doc, "/manifest/properties/speed", "decimals").range, json!([0, 6]));
    let group = field(&doc, "/manifest/properties/speed", "group");
    assert_eq!((group.widget.as_str(), group.options.clone()), ("ref:group", vec![json!("tools")]));
    let icon = field(&doc, "/manifest/properties/speed", "icon");
    assert!(icon.widget == "ref:icon" && icon.options.contains(&json!("weather-snow")));
    let mood = field(&doc, "/manifest/properties/mood", "default");
    assert_eq!(
        (mood.widget.as_str(), mood.options.clone()),
        ("ref:option", vec![json!("calm"), json!("storm")])
    );
    assert_eq!(mood.names, ["Calm", "Storm"]);
    let options = field(&doc, "/manifest/properties/mood", "options");
    assert_eq!(options.sample, json!({"value": "option_3", "label": ""}));
    assert_eq!(field(&doc, "/manifest/properties/mood/options/0", "value").widget, "scalar");
    assert_eq!(field(&doc, "/manifest/properties/tint", "default").widget, "rgb");
    doc["manifest"]["properties"]["tint"]["alpha"] = json!(true);
    assert_eq!(field(&doc, "/manifest/properties/tint", "default").widget, "color");
    let accept = field(&doc, "/manifest/properties/photo", "accept");
    assert_eq!(
        (accept.widget.as_str(), accept.options.clone()),
        ("flags", vec![json!("image"), json!("video")])
    );
    let asset = field(&doc, "/manifest/properties/photo", "default_asset");
    assert_eq!((asset.widget.as_str(), asset.options.len()), ("ref:media", 2));
    doc["manifest"]["properties"]["shots"] = json!({"type": "folder"});
    assert_eq!(
        field(&doc, "/manifest/properties/shots", "pick").options,
        [json!("random"), json!("sequence")]
    );
    let every = field(&doc, "/manifest/properties/shots", "every");
    assert_eq!((every.unit.as_str(), every.range.clone()), ("s", json!([10, 86400])));
    assert_eq!(field(&doc, "/manifest/properties/secret", "secret").widget, "bool");
    assert_eq!(field(&doc, "/manifest/properties/tools", "collapsed").widget, "bool");
}

#[test]
fn presets_and_variants_get_one_row_per_value() {
    let mut doc = sample();
    run(&mut doc, json!({"add_preset": {"label": "Quiet"}}));
    assert_eq!(doc["manifest"]["presets"][1], json!({"id": "preset_1", "label": "Quiet", "values": {}}));
    assert!(refused(&doc, json!({"add_preset": {"id": "wild"}})).contains("exists"));
    let rows: Vec<String> = fields(&doc, "/manifest/presets/0").iter().map(|f| f.key.clone()).collect();
    assert_eq!(rows, ["id", "label", "values", "tint", "mood", "speed"]);
    let mood = field(&doc, "/manifest/presets/0", "mood");
    assert_eq!((mood.widget.as_str(), mood.set, mood.title.as_str()), ("ref:option", true, "Mood"));
    let speed = field(&doc, "/manifest/presets/0", "speed");
    assert_eq!((speed.widget.as_str(), speed.set, speed.range.clone()), ("number", false, json!([0.5, 4.0])));
    run(&mut doc, json!({"set": {"pointer": "/manifest/presets/0/values/speed", "value": 3}}));
    assert_eq!(errors(&doc), []);
    let sky = fields(&doc, "/scene/layers/0/image");
    assert!(sky.iter().all(|f| f.pointer != "/scene/layers/0/image/map" || f.widget == "group"));
    let storm = field(&doc, "/scene/layers/0/image", "storm");
    assert_eq!((storm.widget.as_str(), storm.set, storm.title.as_str()), ("ref:image", false, "Storm"));
    assert_eq!(storm.options, [json!("calm"), json!("storm")]);
    run(&mut doc, json!({"set": {"pointer": storm.pointer, "value": "storm"}}));
    assert_eq!(doc["scene"]["layers"][0]["image"]["map"], json!({"calm": "calm", "storm": "storm"}));
    assert!(field(&doc, "/scene/layers/0/image", "storm").set);
}
