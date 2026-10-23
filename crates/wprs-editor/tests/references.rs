use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_editor::Problem;
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::check;
use wprs_editor::inspect::{Field, fields};
use wprs_editor::listing::{presets, rows};
use wprs_editor::ops::apply;
use wprs_editor::references::scripts;
use wprs_editor::starters::starter;
use wprs_scene::Property;

fn run(doc: &mut Value, value: Value) {
    let ops = plan(doc, &serde_json::from_value::<Action>(value).unwrap()).unwrap();
    apply(doc, &ops).unwrap();
}

fn refused(doc: &Value, value: Value) -> String {
    plan(doc, &serde_json::from_value::<Action>(value).unwrap()).unwrap_err().to_string()
}

fn field(doc: &Value, pointer: &str, key: &str) -> Field {
    fields(doc, pointer).into_iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{pointer} {key}"))
}

fn errors(doc: &Value) -> Vec<Problem> {
    check(doc).into_iter().filter(|p| !p.warning).collect()
}

fn sample() -> Value {
    let mut doc = starter("empty", "org.example.refs", "Refs", "en").unwrap();
    doc["manifest"]["strings"] =
        json!({"en": {"mood": "Weather", "calm": "Calm sky", "storm": "Stormy sky"}});
    doc["manifest"]["properties"] = json!({
        "look": {"type": "group", "label": "Look", "order": 1},
        "mood": {"type": "combo", "label": "@mood", "group": "look", "order": 1, "default": "calm",
            "options": [{"value": "calm", "label": "@calm"}, {"value": "storm", "label": "@storm"}]},
        "rain": {"type": "bool", "default": false, "visible_if": {"mood": "storm"}, "order": 2},
        "wind": {"type": "slider", "min": 0, "max": 1, "default": 0.5, "group": "look", "order": 2},
        "go": {"type": "button", "order": 3},
        "line": {"type": "divider", "order": 4}
    });
    doc["manifest"]["presets"] =
        json!([{"id": "wild", "label": "@storm", "values": {"mood": "storm", "wind": 1}}]);
    doc["scene"]["assets"] =
        json!({"calm": {"path": "assets/calm.png"}, "storm": {"path": "assets/storm.png"}});
    doc["scene"]["layers"] = json!([{"id": "sky", "type": "image", "image": "calm",
        "mask": {"texture": {"prop": "mood", "map": {"calm": "calm"}}}}]);
    doc
}

#[test]
fn removing_a_property_drops_its_values_and_refuses_while_something_names_it() {
    let mut doc = sample();
    run(&mut doc, json!({"set": {"pointer": "/manifest/properties/go/visible_if", "value": "wind > 0.2"}}));
    run(
        &mut doc,
        json!({"set": {"pointer": "/scene/timelines", "value": [{"id": "t", "duration": 1, "trigger": "prop:wind"}]}}),
    );
    run(&mut doc, json!({"set": {"pointer": "/scene/layers/0/opacity", "value": {"prop": "wind"}}}));
    let named = refused(&doc, json!({"remove_property": {"id": "wind"}}));
    for pointer in
        ["/manifest/properties/go/visible_if", "/scene/timelines/0/trigger", "/scene/layers/0/opacity"]
    {
        assert!(named.contains(pointer), "{named}");
    }
    assert!(
        refused(&doc, json!({"remove_property": {"id": "mood"}})).contains("/scene/layers/0/mask/texture")
    );
    run(&mut doc, json!({"set": {"pointer": "/scene/layers/0/mask", "value": {"texture": "storm"}}}));
    run(&mut doc, json!({"remove_property": {"id": "mood"}}));
    let props = &doc["manifest"]["properties"];
    assert!(props.get("mood").is_none() && props["rain"].get("visible_if").is_none());
    assert_eq!(doc["manifest"]["presets"][0]["values"], json!({"wind": 1}));
    run(&mut doc, json!({"remove_property": {"id": "go"}}));
    run(&mut doc, json!({"remove": {"pointer": "/scene/layers/0/opacity"}}));
    run(&mut doc, json!({"remove": {"pointer": "/scene/timelines"}}));
    run(&mut doc, json!({"remove_property": {"id": "look"}}));
    assert!(doc["manifest"]["properties"]["wind"].get("group").is_none());
    assert_eq!(errors(&doc), []);
    assert!(refused(&doc, json!({"remove_property": {"id": "gone"}})).contains("no property"));
    let mut bare = starter("empty", "org.example.bare", "Bare", "en").unwrap();
    run(&mut bare, json!({"add_property": {"kind": "bool", "id": "rain"}}));
    let before = bare["manifest"].clone();
    run(&mut bare, json!({"remove_property": {"id": "rain"}}));
    assert_eq!(bare["manifest"].get("presets"), before.get("presets"));
    assert_eq!(bare["manifest"]["properties"], json!({}));
}

#[test]
fn problems_of_conditions_and_presets_point_at_what_to_fix() {
    let mut doc = sample();
    run(&mut doc, json!({"remove": {"pointer": "/manifest/properties/mood"}}));
    let found = errors(&doc);
    assert_eq!(found[0].pointer, "/manifest/properties/rain/visible_if", "{found:?}");
    assert!(!found[0].message.contains("presets"), "{found:?}");
    run(&mut doc, json!({"remove": {"pointer": "/manifest/properties/rain/visible_if"}}));
    assert_eq!(errors(&doc)[0].pointer, "/manifest/presets/0/values/mood");
}

#[test]
fn texts_named_by_key_show_the_strings_of_the_manifest() {
    let doc = sample();
    let mood = rows(&doc).into_iter().find(|r| r["id"] == "mood").unwrap();
    assert_eq!(mood["label"], "Weather");
    assert_eq!(presets(&doc)[0]["label"], "Stormy sky");
    assert_eq!(field(&doc, "/manifest/properties/mood", "default").names, ["Calm sky", "Stormy sky"]);
    let preset = field(&doc, "/manifest/presets/0", "mood");
    assert_eq!(
        (preset.title.as_str(), preset.names.clone()),
        ("Weather", vec!["Calm sky".into(), "Stormy sky".into()])
    );
    let calm = field(&doc, "/scene/layers/0/mask/texture", "calm");
    assert_eq!(calm.title, "Calm sky");
}

#[test]
fn references_offer_only_what_the_validator_accepts() {
    let doc = sample();
    let group = field(&doc, "/manifest/properties/rain", "group");
    assert_eq!((group.options.clone(), group.names.clone()), (vec![json!("look")], vec!["Look".to_string()]));
    assert_eq!(field(&doc, "/manifest/properties/look", "group").options, Vec::<Value>::new());
    let prop = field(&doc, "/scene/layers/0/mask/texture", "prop");
    assert_eq!((prop.widget.as_str(), prop.value.clone()), ("ref:property", json!("mood")));
    assert_eq!(prop.options, [json!("mood"), json!("wind"), json!("rain")]);
    assert_eq!(prop.names, ["Weather", "wind", "rain"]);
    assert!(
        fields(&doc, "/scene/layers/0/mask/texture")
            .iter()
            .all(|f| f.pointer != "/scene/layers/0/mask/texture")
    );
    let mut doc = doc;
    run(&mut doc, json!({"set": {"pointer": "/scene/timelines", "value": [{"id": "t", "duration": 1}]}}));
    let trigger = field(&doc, "/scene/timelines/0", "trigger");
    assert_eq!(trigger.widget, "ref:trigger");
    assert_eq!(
        trigger.options,
        ["start", "beat", "prop:mood", "prop:wind", "prop:rain", "prop:go"].map(Value::from)
    );
    assert_eq!(trigger.names[2], "Weather");
}

#[test]
fn the_list_follows_the_settings_form_and_new_combos_carry_no_words() {
    let mut doc = sample();
    run(&mut doc, json!({"add_property": {"kind": "combo"}}));
    let combo = &doc["manifest"]["properties"]["combo_1"];
    assert_eq!(
        combo["options"],
        json!([{"value": "option_1", "label": ""}, {"value": "option_2", "label": ""}])
    );
    assert_eq!(field(&doc, "/manifest/properties/combo_1", "default").names, ["option_1", "option_2"]);
    let props: BTreeMap<String, Property> =
        serde_json::from_value(doc["manifest"]["properties"].clone()).unwrap();
    let (form, _) = wprs_scene::form(&props, &[], &wprs_scene::effective(&props, &[]), &[]);
    let listed: Vec<Value> = rows(&doc).iter().map(|r| r["id"].clone()).collect();
    assert_eq!(listed, form.iter().map(|f| json!(f.key)).collect::<Vec<_>>());
    assert_eq!(listed[..3], [json!("look"), json!("mood"), json!("wind")]);
    let depths: Vec<Value> = rows(&doc).iter().map(|r| r["depth"].clone()).collect();
    assert_eq!(depths[..4], [json!(0), json!(1), json!(1), json!(0)]);
    let combo = form.iter().find(|f| f.key == "combo_1").unwrap();
    assert_eq!((combo.label.as_str(), combo.options[0].label.as_str()), ("combo_1", "option_1"));
}

#[test]
fn scripts_that_name_a_property_are_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("scripts")).unwrap();
    std::fs::write(tmp.path().join("scripts/a.js"), "export const f = () => props.get('wind');").unwrap();
    std::fs::write(tmp.path().join("scripts/b.js"), "export const g = () => props.get(\"windy\");").unwrap();
    let mut doc = sample();
    doc["scene"]["scripts"] =
        json!([{"path": "scripts/a.js"}, {"path": "scripts/b.js"}, {"path": "../x.js"}]);
    assert_eq!(scripts(tmp.path(), &doc, "wind"), ["scripts/a.js"]);
    assert_eq!(scripts(tmp.path(), &doc, "windy"), ["scripts/b.js"]);
    assert!(scripts(tmp.path(), &doc, "mood").is_empty());
}
