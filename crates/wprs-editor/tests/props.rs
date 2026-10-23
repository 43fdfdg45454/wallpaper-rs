use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::check;
use wprs_editor::inspect::{Field, fields};
use wprs_editor::listing::rows;
use wprs_editor::ops::apply;
use wprs_editor::props::PROPERTY_KINDS;
use wprs_editor::settings::condition;
use wprs_editor::starters::starter;
use wprs_editor::{Problem, Project};

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

fn action(value: Value) -> Action {
    serde_json::from_value(value).unwrap()
}

fn run(doc: &mut Value, value: Value) {
    let ops = plan(doc, &action(value)).unwrap();
    apply(doc, &ops).unwrap();
}

fn refused(doc: &Value, value: Value) -> String {
    plan(doc, &action(value)).unwrap_err().to_string()
}

fn field(doc: &Value, pointer: &str, key: &str) -> Field {
    fields(doc, pointer).into_iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{pointer} {key}"))
}

fn errors(doc: &Value) -> Vec<Problem> {
    check(doc).into_iter().filter(|p| !p.warning).collect()
}

fn sample() -> Value {
    let mut doc = starter("empty", "org.example.kinds", "Kinds", "en").unwrap();
    doc["manifest"]["properties"] = json!({
        "mood": {"type": "combo", "label": "Mood", "options": [{"value": "calm", "label": "Calm"}, {"value": "storm", "label": "Storm"}],
            "default": "calm", "order": 1},
        "speed": {"type": "slider", "min": 0.5, "max": 4, "step": 0.5, "default": 1, "unit": "x", "order": 2,
            "visible_if": "mood == 'storm' && tint != '#000000'"},
        "go": {"type": "button", "enabled_if": {"mood": "storm"}, "group": "tools", "order": 1},
        "tools": {"type": "group", "order": 3},
        "tint": {"type": "color", "default": "#ffffff"},
        "photo": {"type": "file", "accept": ["image"]},
        "secret": {"type": "text", "secret": true, "visible_if": "speed > 1"}
    });
    doc["manifest"]["presets"] = json!([{"id": "wild", "label": "Wild", "values": {"mood": "storm"}}]);
    doc["scene"]["assets"] =
        json!({"calm": {"path": "assets/calm.png"}, "storm": {"path": "assets/storm.png"}});
    doc["scene"]["timelines"] = json!([{"id": "pulse", "duration": 1, "trigger": "prop:go"}]);
    doc["scene"]["layers"] =
        json!([{"id": "sky", "type": "image", "image": {"prop": "mood", "map": {"calm": "calm"}}}]);
    doc
}

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
