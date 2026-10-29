use crate::ops::escape;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_scene::{Control, Property, Text};

pub fn label(doc: &Value, text: &Value, fallback: &str) -> String {
    let m = &doc["manifest"];
    let key = text.as_str().and_then(|t| t.strip_prefix('@')).filter(|_| m["schema"] == wprs_format::SCHEMA);
    let strings = m["strings"].as_object().into_iter().flatten();
    let text = match key {
        Some(key) => {
            Value::Object(strings.filter_map(|(l, t)| Some((l.clone(), t.get(key)?.clone()))).collect())
        }
        None => text.clone(),
    };
    let mut languages = wprs_core::locale::languages();
    languages.extend(m["default_locale"].as_str().map(String::from));
    let picked = Text::deserialize(&text).map(|t| t.pick(&languages));
    picked.ok().filter(|t| !t.is_empty()).unwrap_or_else(|| fallback.into())
}

pub fn named(doc: &Value, id: &str) -> String {
    label(doc, &doc["manifest"]["properties"][id]["label"], id)
}

pub fn values(doc: &Value, prop: &Value) -> Vec<(Value, String)> {
    let options = prop["options"].as_array().into_iter().flatten();
    let plain = |v: &Value| v.as_str().map_or_else(|| v.to_string(), String::from);
    let pair = |o: &Value| match o.get("value") {
        Some(v) if o.get("label").is_some() => (v.clone(), label(doc, &o["label"], &plain(v))),
        _ => (o.clone(), plain(o)),
    };
    options.map(pair).collect()
}

pub fn ordered(doc: &Value) -> Vec<String> {
    let all = doc["manifest"]["properties"].as_object().into_iter().flatten();
    let rough = |p: &Value| {
        let (group, order) = (p["group"].as_str().map(String::from), p["order"].as_f64().unwrap_or(0.0));
        Property { group, order, ..Property::new(Control::Divider {}) }
    };
    let props: BTreeMap<String, Property> = all.map(|(id, p)| (id.clone(), rough(p))).collect();
    let mut list: Vec<_> = props.iter().collect();
    list.sort_by(|a, b| wprs_scene::order(&props, *a, *b));
    list.into_iter().map(|(id, _)| id.clone()).collect()
}

pub fn rows(doc: &Value) -> Vec<Value> {
    let all = &doc["manifest"]["properties"];
    let ids = ordered(doc);
    let groups: Vec<Option<&str>> =
        ids.iter().map(|id| all[id]["group"].as_str().filter(|g| all[*g]["type"] == "group")).collect();
    let row = |(i, id): (usize, &String)| {
        let siblings: Vec<&String> =
            ids.iter().zip(&groups).filter(|(_, g)| **g == groups[i]).map(|(s, _)| s).collect();
        let index = siblings.iter().position(|s| *s == id).unwrap_or_default();
        json!({"id": id, "type": all[id]["type"], "label": named(doc, id), "group": groups[i],
            "depth": usize::from(groups[i].is_some()), "index": index, "count": siblings.len(),
            "pointer": format!("/manifest/properties/{}", escape(id))})
    };
    ids.iter().enumerate().map(row).collect()
}

pub fn presets(doc: &Value) -> Vec<Value> {
    let list = doc["manifest"]["presets"].as_array().into_iter().flatten().enumerate();
    let row = |(i, p): (usize, &Value)| {
        let id = p["id"].as_str().unwrap_or_default();
        json!({"id": id, "label": label(doc, &p["label"], id), "pointer": format!("/manifest/presets/{i}")})
    };
    list.map(row).collect()
}
