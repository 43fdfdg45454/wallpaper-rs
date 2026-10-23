use serde_json::{Value, json};
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::check;
use wprs_editor::inspect::fields;
use wprs_editor::ops::apply;
use wprs_editor::starters::{default_id, slug, starter};

fn act(doc: &mut Value, action: Value) -> anyhow::Result<()> {
    let action: Action = serde_json::from_value(action).unwrap();
    apply(doc, &plan(doc, &action)?).map(drop)
}

fn clock() -> Value {
    let mut doc = starter("clock", "org.example.test", "Test", "en").unwrap();
    let show = json!({"type": "bool", "label": "Show", "default": true});
    act(&mut doc, json!({"set": {"pointer": "/manifest/properties/show", "value": show}})).unwrap();
    doc
}

#[test]
fn hiding_and_showing_again_keeps_bindings_and_keyframes() {
    let mut doc = clock();
    let seconds = "/scene/layers/2/visible";
    act(&mut doc, json!({"set": {"pointer": seconds, "value": {"prop": "show"}}})).unwrap();
    act(&mut doc, json!({"hide": {"id": "seconds", "hidden": true}})).unwrap();
    assert_eq!(doc.pointer(seconds), Some(&json!(false)));
    act(&mut doc, json!({"hide": {"id": "seconds", "hidden": false}})).unwrap();
    assert_eq!(doc.pointer(seconds), Some(&json!({"prop": "show"})));
    assert_eq!(doc["scene"]["ext"]["editor"]["hidden"], json!({}));
    let keyed = json!({"keys": [[0.0, true], [2.0, false]]});
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/1/visible", "value": keyed}})).unwrap();
    act(&mut doc, json!({"hide": {"id": "time", "hidden": true}})).unwrap();
    act(&mut doc, json!({"duplicate_layer": {"id": "time"}})).unwrap();
    act(&mut doc, json!({"hide": {"id": "time_copy", "hidden": false}})).unwrap();
    assert_eq!(doc.pointer("/scene/layers/2/visible"), Some(&keyed));
    act(&mut doc, json!({"remove_layer": {"id": "time"}})).unwrap();
    assert_eq!(doc["scene"]["ext"]["editor"]["hidden"], json!({}));
    assert_eq!(check(&doc), []);
}

#[test]
fn groups_take_each_layer_once_and_ungroup_only_when_nothing_is_lost() {
    let mut doc = clock();
    act(&mut doc, json!({"group": {"ids": ["sky", "sky"], "id": "g"}})).unwrap();
    let ids = |doc: &Value, list: &str| -> Vec<String> {
        doc.pointer(list)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["id"].as_str().unwrap().into())
            .collect()
    };
    assert_eq!(
        (ids(&doc, "/scene/layers"), ids(&doc, "/scene/layers/0/layers")),
        (vec!["g".into(), "time".into(), "seconds".into()], vec!["sky".into()])
    );
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/0/opacity", "value": 0.3}})).unwrap();
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/0/transform/position", "value": [300, 0]}}))
        .unwrap();
    let refused = act(&mut doc, json!({"ungroup": {"id": "g"}})).unwrap_err().to_string();
    assert!(refused.contains("opacity") && refused.contains("transform"), "{refused}");
    act(&mut doc, json!({"remove": {"pointer": "/scene/layers/0/opacity"}})).unwrap();
    act(&mut doc, json!({"remove": {"pointer": "/scene/layers/0/transform"}})).unwrap();
    act(&mut doc, json!({"ungroup": {"id": "g"}})).unwrap();
    assert_eq!(ids(&doc, "/scene/layers"), ["sky", "time", "seconds"]);
}

#[test]
fn problems_of_nested_layers_point_at_them() {
    let mut doc = clock();
    act(&mut doc, json!({"group": {"ids": ["time", "seconds"], "id": "g1"}})).unwrap();
    act(&mut doc, json!({"set": {"pointer": "/scene/layers/1/layers/1/id", "value": "Bad Id"}})).unwrap();
    let found = check(&doc);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pointer, "/scene/layers/1/layers/1/id");
    assert!(doc.pointer(&found[0].pointer).is_some());
}

#[test]
fn lists_of_pairs_get_fitting_widgets() {
    let doc = starter("particles", "org.example.test", "Test", "en").unwrap();
    let widget = |pointer: &str, key: &str| {
        fields(&doc, pointer).into_iter().find(|f| f.key == key).map(|f| f.widget).unwrap_or_default()
    };
    assert_eq!(widget("/scene/layers/0", "stops"), "stops");
    assert_eq!(widget("/scene/particles/flakes/emitters/0", "box"), "points");
    assert_eq!(widget("/scene/particles/flakes", "size_over_life"), "curve");
}

#[test]
fn names_lose_their_accents_in_ids_and_folders() {
    assert_eq!(slug("Canción del mar"), "cancion-del-mar");
    assert_eq!(slug("Año nuevo, ¡ya!"), "ano-nuevo-ya");
    assert_eq!(default_id("Über Straße"), "org.example.uber-strasse");
}
