use super::*;

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
        (vec!["properties2"], vec!["properties2", "folder"], vec!["properties2", "user_videos"])
    );
    let plain: BTreeMap<String, Property> = serde_json::from_str(include_str!("../props.json")).unwrap();
    assert!(plain.values().all(|p| p.features().is_empty()));
    let raw: BTreeMap<String, Value> = serde_json::from_str(include_str!("../props.json")).unwrap();
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
