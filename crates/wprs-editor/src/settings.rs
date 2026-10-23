use crate::inspect::Field;
use crate::layers::assets_of;
use crate::listing::{named, ordered, values};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_scene::{Condition, Property, cycle};

pub const ORDER: &str = "label help group order icon visible_if enabled_if min max step default decimals \
    integer unit editable log options alpha max_len multiline secret accept default_asset pick every collapsed";

pub fn properties(doc: &Value) -> BTreeMap<String, Property> {
    let all = doc["manifest"]["properties"].as_object().into_iter().flatten();
    all.filter_map(|(id, p)| Some((id.clone(), Property::deserialize(p).ok()?))).collect()
}

pub fn owner(pointer: &str) -> Option<(&str, &str)> {
    let rest = pointer.strip_prefix("/manifest/properties/")?;
    Some(rest.split_once('/').unwrap_or((rest, "")))
}

pub fn parse(text: &str) -> Condition {
    let text = text.trim();
    match serde_json::from_str(text) {
        Ok(map) => Condition::Equal(map),
        Err(_) if text.is_empty() => Condition::default(),
        Err(_) => Condition::Expr(text.into()),
    }
}

pub fn condition(doc: &Value, pointer: &str, text: &str) -> Option<String> {
    let (id, key) = owner(pointer)?;
    let enabled = key == "enabled_if";
    let mut props = properties(doc);
    let prop = props.get_mut(id)?;
    *(if enabled { &mut prop.enabled_if } else { &mut prop.visible_if }) = parse(text);
    let own = if enabled { &props[id].enabled_if } else { &props[id].visible_if };
    let looped = || cycle(&props).map(|c| format!("makes the conditions of {c} depend on themselves"));
    own.check(&props, id).err().or_else(looped)
}

pub fn candidates(doc: &Value, reference: &str, pointer: &str) -> Option<Vec<Value>> {
    let own = owner(pointer).map(|(id, _)| id);
    let kind = |id: &str| doc["manifest"]["properties"][id]["type"].as_str().unwrap_or_default().to_owned();
    let valued = |button: bool| {
        let props = properties(doc);
        let holds = move |id: &String| {
            props.get(id).is_some_and(|p| p.holds_value() || (button && p.kind() == "button"))
        };
        ordered(doc).into_iter().filter(holds)
    };
    Some(match reference {
        "group" if own.is_some_and(|id| kind(id) == "group") => Vec::new(),
        "group" => ordered(doc)
            .into_iter()
            .filter(|id| kind(id) == "group" && own != Some(id))
            .map(Value::from)
            .collect(),
        "property" => valued(false).map(Value::from).collect(),
        "trigger" => {
            let props = valued(true).map(|id| format!("prop:{id}"));
            ["start".into(), "beat".into()].into_iter().chain(props).map(Value::String).collect()
        }
        "icon" => wprs_scene::ICONS.split_whitespace().map(Value::from).collect(),
        "option" => values(doc, &doc["manifest"]["properties"][own?]).into_iter().map(|(v, _)| v).collect(),
        "media" => ["image", "video"].iter().flat_map(|k| assets_of(doc, k)).map(Value::from).collect(),
        _ => return None,
    })
}

pub fn names(doc: &Value, reference: &str, options: &[Value]) -> Vec<String> {
    let name = |o: &Value| {
        let text = o.as_str().unwrap_or_default();
        match reference {
            "trigger" => text.strip_prefix("prop:").map(|id| named(doc, id)).unwrap_or_default(),
            _ => named(doc, text),
        }
    };
    match reference {
        "group" | "property" | "trigger" => options.iter().map(name).collect(),
        _ => Vec::new(),
    }
}

pub fn adjust(doc: &Value, f: &mut Field) {
    let Some((id, rest)) = owner(&f.pointer) else { return };
    let prop = &doc["manifest"]["properties"][id];
    match (rest, prop["type"].as_str().unwrap_or_default()) {
        ("default", "slider") if prop["min"].is_number() && prop["max"].is_number() => {
            (f.range, f.unit) =
                (json!([prop["min"], prop["max"]]), prop["unit"].as_str().unwrap_or("").into());
            if prop["integer"] == true {
                f.widget = "integer".into();
            }
        }
        ("default", "color") if prop["alpha"] != true => f.widget = "rgb".into(),
        ("default", "combo") => f.names = values(doc, prop).into_iter().map(|(_, n)| n).collect(),
        ("options", "combo") => f.sample = next(doc, prop),
        _ => {}
    }
}

fn next(doc: &Value, prop: &Value) -> Value {
    let found = values(doc, prop);
    let value = match found.first() {
        Some((v, _)) if v.is_number() => {
            json!(found.iter().filter_map(|(v, _)| v.as_f64()).fold(0.0, f64::max) as i64 + 1)
        }
        _ => json!(
            (found.len() + 1..)
                .map(|i| format!("option_{i}"))
                .find(|s| !found.iter().any(|(v, _)| v == s))
                .unwrap_or_default()
        ),
    };
    json!({"value": value, "label": ""})
}
