use super::{literals, qml_files};
use crate::editing::describe;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use wprs_core::edit::{ACTIONS, Edit, Source};

fn runs(scenes: bool) -> (String, Vec<String>) {
    let files = qml_files().into_iter().filter(|(name, _)| name.starts_with("Scene") == scenes);
    let text: String = files.map(|(_, text)| text).collect();
    let used = literals(&text, ".run(").into_iter().map(String::from).collect();
    (text, used)
}

#[test]
fn every_scene_editor_action_is_a_wprs_ctl_scene_command() {
    let (text, used) = runs(true);
    for action in &used {
        assert!(wprs_editor::ACTIONS.contains(&action.as_str()), "{action} is not a wprs-ctl scene command");
    }
    for action in wprs_editor::ACTIONS {
        assert!(text.contains(&format!("\"{action}\"")), "the scene editor never runs {action}");
    }
}

#[test]
fn every_editor_action_is_a_command_line_action() {
    let (_, used) = runs(false);
    for action in ACTIONS {
        assert!(used.iter().any(|u| u == action), "the editor never runs {action}");
    }
    for action in &used {
        assert!(ACTIONS.contains(&action.as_str()), "{action} is not a wprs-ctl edit command");
    }
}

#[test]
fn the_editor_describes_what_its_controls_show() {
    let mut edit = Edit { source: Source { pixels: [4000, 2000], ..Source::default() }, ..Edit::default() };
    let values = |edit: &Edit| serde_json::from_str::<Value>(&describe(edit)).unwrap();
    let blank = values(&edit);
    assert_eq!((&blank["levels"]["temperature"], &blank["levels"]["ev"]), (&json!(6504.0), &json!(0.0)));
    assert_eq!((&blank["frame"], &blank["zoom"]), (&json!([4000, 2000]), &json!(1.0)));
    assert_eq!(blank["tools"]["vi"]["midpoint"], json!(0.5));
    assert_eq!(
        (&blank["tools"]["ov"]["opacity"], &blank["curves"]["g"]["samples"][32]),
        (&json!(1.0), &json!(0.5))
    );
    edit.set("geometry.rotate", "90").unwrap();
    edit.set("ev", "1").unwrap();
    edit.act(
        serde_json::from_value(
            json!({"mask": {"name": "sky", "spec": "radial:0.5,0.5,0.25,0.25", "invert": true}}),
        )
        .unwrap(),
    )
    .unwrap();
    let turned = values(&edit);
    assert_eq!((&turned["frame"], &turned["levels"]["ev"]), (&json!([2000, 4000]), &json!(1.0)));
    assert_eq!(
        turned["masks"]["sky"],
        json!({"kind": "radial", "values": [0.5, 0.5, 0.25, 0.25, 0.5], "invert": true})
    );
    assert!(edit.ops.len() == 1 && turned["tools"]["bl"]["radius"] == json!(0.0));
    assert_eq!((&blank["depth"]["input"], &blank["parallax"]["enabled"]), (&json!(518), &json!(false)));
    edit.act(serde_json::from_value(json!({"parallax": {"source": "pointer"}})).unwrap()).unwrap();
    let moving = values(&edit);
    assert_eq!(
        (&moving["parallax"]["enabled"], &moving["parallax"]["source"]),
        (&json!(true), &json!("pointer"))
    );
}

fn named(node: &Value, keys: &mut BTreeSet<String>, values: &mut BTreeSet<String>) {
    let texts =
        |list: &Vec<Value>| list.iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>();
    match node {
        Value::Object(map) => map.iter().for_each(|(key, value)| {
            match (key.as_str(), value) {
                ("properties", Value::Object(fields))
                    if !fields.get("type").is_some_and(Value::is_string) =>
                {
                    keys.extend(fields.keys().cloned())
                }
                ("required", Value::Array(list)) => keys.extend(texts(list)),
                ("enum", Value::Array(list)) => values.extend(texts(list)),
                ("const", Value::String(text)) => drop(values.insert(text.clone())),
                _ => {}
            }
            named(value, keys, values);
        }),
        Value::Array(list) => list.iter().for_each(|value| named(value, keys, values)),
        _ => {}
    }
}

#[test]
fn the_scene_editor_has_words_for_every_field_value_and_effect() {
    let facades = qml_files()
        .into_iter()
        .filter(|(name, _)| ["SceneWords.qml", "ManifestWords.qml"].contains(&name.as_str()));
    let words: String = facades.map(|(_, text)| text).collect();
    assert!(words.contains("ManifestWords.words[text]"), "SceneWords.qml falls back to ManifestWords.qml");
    let (mut keys, mut values) = (BTreeSet::new(), BTreeSet::new());
    named(&wprs_scene::schema(), &mut keys, &mut values);
    named(&wprs_scene::property_schema(), &mut keys, &mut values);
    named(&wprs_format::manifest_schema(), &mut keys, &mut values);
    for effect in wprs_scene::builtin() {
        keys.insert(effect.id.clone());
        keys.extend(effect.params.iter().map(|p| p.id.clone()));
    }
    let easings = wprs_scene::easings();
    let curves = easings.iter().map(|e| e.rsplit('_').next().unwrap_or_default().to_owned());
    values.retain(|v| !easings.contains(v));
    let wanted: BTreeSet<String> = keys.into_iter().chain(values).chain(curves).collect();
    let missing: Vec<&String> =
        wanted.iter().filter(|k| !words.contains(&format!("\"{k}\": App.tr("))).collect();
    assert!(missing.is_empty(), "SceneWords.qml and ManifestWords.qml have no words for {missing:?}");
    for source in ["keys", "prop", "audio", "cursor", "clock", "script"] {
        assert!(words.contains(&format!("\"{source}\": App.tr(")), "{source}");
    }
}
