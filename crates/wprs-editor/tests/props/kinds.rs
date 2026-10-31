use super::*;

#[test]
fn every_kind_of_property_starts_valid_and_the_features_follow() {
    let root = tempfile::tempdir().unwrap();
    state();
    let doc = starter("empty", "org.example.kinds", "Kinds", "en").unwrap();
    let mut project = Project::create(&root.path().join("kinds"), doc).unwrap();
    for kind in PROPERTY_KINDS {
        project.act(&action(json!({"add_property": {"kind": kind, "label": kind}}))).unwrap();
    }
    let added: Vec<Value> = rows(&project.doc).iter().map(|r| r["id"].clone()).collect();
    assert_eq!(added.len(), PROPERTY_KINDS.len());
    assert_eq!(added[0], "slider_1");
    assert_eq!(errors(&project.doc), []);
    assert_eq!(project.doc["manifest"]["requires"]["features"], json!(["folder", "properties2"]));
    let remove = |project: &mut Project, id: &str| {
        project.act(&action(json!({"remove": {"pointer": format!("/manifest/properties/{id}")}}))).unwrap();
    };
    remove(&mut project, "folder_1");
    assert_eq!(project.doc["manifest"]["requires"]["features"], json!(["properties2"]));
    remove(&mut project, "button_1");
    remove(&mut project, "divider_1");
    assert!(project.doc["manifest"].get("requires").is_none());
    let doc = &project.doc;
    assert!(refused(doc, json!({"add_property": {"kind": "knob"}})).contains("not a kind of property"));
    assert!(refused(doc, json!({"add_property": {"kind": "bool", "id": "Slider_1"}})).contains("in use"));
    assert!(refused(doc, json!({"add_property": {"kind": "bool", "id": "2fast"}})).contains("letter"));
}

#[test]
fn properties_move_within_and_between_groups() {
    let mut doc = sample();
    let order =
        |doc: &Value| rows(doc).iter().map(|r| r["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(order(&doc), ["photo", "secret", "tint", "mood", "speed", "tools", "go"]);
    run(&mut doc, json!({"move_property": {"id": "tools", "to": 0}}));
    assert_eq!(order(&doc), ["tools", "go", "photo", "secret", "tint", "mood", "speed"]);
    run(&mut doc, json!({"move_property": {"id": "speed", "to": 0, "group": "tools"}}));
    assert_eq!(order(&doc), ["tools", "speed", "go", "photo", "secret", "tint", "mood"]);
    assert_eq!(rows(&doc)[1]["depth"], 1);
    run(&mut doc, json!({"move_property": {"id": "go", "to": 9, "group": ""}}));
    assert!(doc["manifest"]["properties"]["go"].get("group").is_none());
    assert_eq!(order(&doc).last().unwrap(), "go");
    run(&mut doc, json!({"add_property": {"kind": "bool", "id": "rain", "group": "tools"}}));
    assert_eq!(doc["manifest"]["properties"]["rain"]["order"], 2);
    assert!(
        refused(&doc, json!({"move_property": {"id": "mood", "to": 0, "group": "tint"}}))
            .contains("not a group")
    );
    run(&mut doc, json!({"add_property": {"kind": "group", "id": "more"}}));
    assert!(
        refused(&doc, json!({"move_property": {"id": "more", "to": 0, "group": "tools"}}))
            .contains("another group")
    );
    assert_eq!(errors(&doc), []);
}

#[test]
fn renaming_a_property_follows_every_place_that_names_it() {
    let mut doc = sample();
    run(&mut doc, json!({"rename_property": {"id": "mood", "to": "weather"}}));
    run(&mut doc, json!({"rename_property": {"id": "go", "to": "flash"}}));
    run(&mut doc, json!({"rename_property": {"id": "tools", "to": "extras"}}));
    let props = &doc["manifest"]["properties"];
    assert!(props.get("mood").is_none() && props["weather"]["type"] == "combo");
    assert_eq!(props["speed"]["visible_if"], "weather == 'storm' && tint != '#000000'");
    assert_eq!(props["flash"]["enabled_if"], json!({"weather": "storm"}));
    assert_eq!(props["flash"]["group"], "extras");
    assert_eq!(doc["manifest"]["presets"][0]["values"], json!({"weather": "storm"}));
    assert_eq!(doc["scene"]["layers"][0]["image"]["prop"], "weather");
    assert_eq!(doc["scene"]["timelines"][0]["trigger"], "prop:flash");
    assert_eq!(errors(&doc), []);
    assert!(refused(&doc, json!({"rename_property": {"id": "speed", "to": "Weather"}})).contains("in use"));
    assert!(refused(&doc, json!({"rename_property": {"id": "gone", "to": "here"}})).contains("no property"));
    run(&mut doc, json!({"rename_property": {"id": "speed", "to": "Speed"}}));
    assert_eq!(doc["manifest"]["properties"]["secret"]["visible_if"], "Speed > 1");
}
