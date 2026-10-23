use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};

pub const PROPERTY_KINDS: [&str; 11] =
    ["slider", "bool", "color", "combo", "text", "file", "folder", "button", "group", "label", "divider"];

pub(crate) fn path(id: &str) -> String {
    format!("/manifest/properties/{}", escape(id))
}

fn template(kind: &str) -> Option<Value> {
    Some(match kind {
        "slider" => json!({"type": kind, "min": 0, "max": 100, "step": 1, "default": 50}),
        "bool" => json!({"type": kind, "default": false}),
        "color" => json!({"type": kind, "default": "#ffffff"}),
        "combo" => {
            json!({"type": kind, "options": [{"value": "option_1", "label": ""}, {"value": "option_2", "label": ""}],
            "default": "option_1"})
        }
        "text" => json!({"type": kind, "default": ""}),
        "file" | "folder" => json!({"type": kind, "accept": ["image"]}),
        "button" | "group" | "label" | "divider" => json!({"type": kind}),
        _ => return None,
    })
}

pub(crate) fn clash(doc: &Value, id: &str, except: &str) -> bool {
    let all = doc["manifest"]["properties"].as_object();
    all.is_some_and(|m| m.keys().any(|k| k != except && k.eq_ignore_ascii_case(id)))
}

pub(crate) fn valid(id: &str) -> Result<()> {
    let valid = id.len() <= 64
        && id.starts_with(|c: char| c.is_ascii_alphabetic())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ensure!(valid, "a property id is a letter followed by at most 63 letters, digits or _");
    Ok(())
}

fn siblings(doc: &Value, group: Option<&str>, except: &str) -> Vec<(String, f64)> {
    let all = doc["manifest"]["properties"].as_object().into_iter().flatten();
    let mut list: Vec<(String, f64)> = all
        .filter(|(id, p)| *id != except && p["group"].as_str() == group)
        .map(|(id, p)| (id.clone(), p["order"].as_f64().unwrap_or(0.0)))
        .collect();
    list.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    list
}

fn target(doc: &Value, group: Option<&str>, kind: &str) -> Result<Option<String>> {
    let Some(g) = group.filter(|g| !g.is_empty()) else { return Ok(None) };
    ensure!(doc["manifest"]["properties"][g]["type"] == "group", "{g} is not a group");
    ensure!(kind != "group", "a group cannot go into another group");
    Ok(Some(g.to_owned()))
}

pub fn add_property(
    doc: &Value,
    kind: &str,
    id: Option<&str>,
    group: Option<&str>,
    label: Option<&str>,
) -> Result<Vec<Op>> {
    let kinds = PROPERTY_KINDS.join(", ");
    let mut value = template(kind).ok_or_else(|| anyhow!("{kind} is not a kind of property: {kinds}"))?;
    let id = match id {
        Some(id) => valid(id).map(|_| id.to_owned())?,
        None => (1..).map(|n| format!("{kind}_{n}")).find(|id| !clash(doc, id, "")).unwrap_or_default(),
    };
    ensure!(!clash(doc, &id, ""), "the id {id} is in use");
    let group = target(doc, group, kind)?;
    let last = siblings(doc, group.as_deref(), &id).iter().map(|(_, o)| *o).fold(0.0, f64::max);
    value["order"] = json!(last.floor() as i64 + 1);
    if let Some(group) = group {
        value["group"] = json!(group);
    }
    if let Some(label) = label.filter(|l| !l.trim().is_empty()) {
        value["label"] = json!(label);
    }
    Ok(set(doc, &path(&id), value))
}

pub fn move_property(doc: &Value, id: &str, to: usize, group: Option<&str>) -> Result<Vec<Op>> {
    let prop = doc.pointer(&path(id)).ok_or_else(|| anyhow!("there is no property {id:?}"))?;
    let group = match group {
        Some(g) => target(doc, Some(g), prop["type"].as_str().unwrap_or_default())?,
        None => prop["group"].as_str().map(String::from),
    };
    let mut list: Vec<String> = siblings(doc, group.as_deref(), id).into_iter().map(|(s, _)| s).collect();
    list.insert(to.min(list.len()), id.to_owned());
    let order = |s: &str| format!("{}/order", path(s));
    let moved = list.iter().enumerate().filter(|(i, s)| doc.pointer(&order(s)) != Some(&json!(i + 1)));
    let mut ops: Vec<Op> = moved.flat_map(|(i, s)| set(doc, &order(s), json!(i + 1))).collect();
    let at = format!("{}/group", path(id));
    match (group, prop.get("group")) {
        (Some(g), now) if now != Some(&json!(g)) => ops.extend(set(doc, &at, json!(g))),
        (None, Some(_)) => ops.push(Op::Remove { path: at }),
        _ => {}
    }
    Ok(ops)
}

pub fn add_preset(doc: &Value, id: Option<&str>, label: Option<&str>) -> Result<Vec<Op>> {
    let list = doc["manifest"]["presets"].as_array();
    let taken = |id: &str| list.into_iter().flatten().any(|p| p["id"] == id);
    let id = match id {
        Some(id) => id.to_owned(),
        None => (1..).map(|n| format!("preset_{n}")).find(|id| !taken(id)).unwrap_or_default(),
    };
    let valid = !id.trim().is_empty() && id.chars().count() <= 64 && !id.chars().any(char::is_control);
    ensure!(valid, "a preset id has 1 to 64 characters");
    ensure!(!taken(&id), "the preset {id} exists");
    let mut preset = json!({"id": id, "values": {}});
    if let Some(label) = label.filter(|l| !l.trim().is_empty()) {
        preset["label"] = json!(label);
    }
    Ok(match list {
        Some(_) => vec![Op::Add { path: "/manifest/presets/-".into(), value: preset }],
        None => set(doc, "/manifest/presets", json!([preset])),
    })
}
