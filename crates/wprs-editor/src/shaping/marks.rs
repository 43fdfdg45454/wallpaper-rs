use super::refused;
use crate::action::{ManifestEdit as M, PropertyEdit};
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;

const DISPLAY: [&str; 7] = ["fit", "zoom", "position", "background", "span", "flip", "accent"];

fn listed(doc: &Value, at: &str, list: Vec<String>) -> Vec<Op> {
    super::effects::put(doc, at, (!list.is_empty()).then(|| json!(list)))
}

fn display(doc: &Value, values: &Map<String, Value>, clear: &[String]) -> Result<Vec<Op>> {
    let mut next = doc["manifest"]["display"].as_object().cloned().unwrap_or_default();
    for key in values.keys().chain(clear) {
        let known = DISPLAY.contains(&key.as_str());
        ensure!(
            known,
            said("editor.no_display", "%1 is not a display setting: %2", &[key, &DISPLAY.join(", ")])
        );
    }
    clear.iter().for_each(|k| drop(next.remove(k)));
    values.iter().for_each(|(k, v)| drop(next.insert(k.clone(), v.clone())));
    Ok(super::effects::put(doc, "/manifest/display", (!next.is_empty()).then_some(Value::Object(next))))
}

pub(crate) fn manifest(dir: &Path, doc: &Value, e: &M) -> Result<Vec<Op>> {
    let m = &doc["manifest"];
    let ops = match e {
        M::Origin { kind, derived_from, provider, source_url } => {
            let mut origin = json!({ "kind": kind });
            if !derived_from.is_empty() {
                origin["derived_from"] = json!(derived_from);
            }
            provider.iter().for_each(|p| origin["provider"] = p.clone());
            source_url.iter().filter(|u| !u.is_empty()).for_each(|u| origin["source_url"] = json!(u));
            set(doc, "/manifest/origin", origin)
        }
        M::License { license } => set(doc, "/manifest/license", json!(license)),
        M::Tags { tags, add, remove } => {
            let old: Vec<String> = m["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str().map(String::from))
                .collect();
            let next = match (add, remove) {
                (true, _) => old.into_iter().chain(tags.iter().cloned()).fold(Vec::new(), |mut l, t| {
                    if !l.contains(&t) {
                        l.push(t)
                    }
                    l
                }),
                (_, true) => old.into_iter().filter(|t| !tags.contains(t)).collect(),
                _ => tags.clone(),
            };
            listed(doc, "/manifest/tags", next)
        }
        M::Authors { authors } => listed(doc, "/manifest/authors", authors.clone()),
        M::Display { values, clear } => display(doc, values, clear)?,
        M::Playback { values, clear } => super::page::section(doc, "playback", values, clear)?,
        M::Web { values, clear } => super::page::section(doc, "web", values, clear)?,
        M::Network { origins, add, remove } => super::page::network(doc, origins, *add, *remove)?,
        M::Inputs { inputs, add, remove } => super::page::inputs(doc, inputs, *add, *remove)?,
        M::AddFile { file, to } => return super::page::add_file(dir, doc, file, to.as_deref()),
    };
    refused(dir, doc, ops)
}

pub(crate) fn property(dir: &Path, doc: &Value, e: &PropertyEdit) -> Result<Vec<Op>> {
    let at = crate::props::path(&e.id);
    let prop = doc
        .pointer(&at)
        .ok_or_else(|| anyhow!(said("editor.there_no_property", "there is no property \"%1\"", &[&e.id])))?;
    let ranged = [("min", e.min), ("max", e.max), ("step", e.step)];
    let slider = prop["type"] == "slider";
    let wanted = ranged.iter().any(|(_, v)| v.is_some());
    ensure!(
        slider || !wanted,
        said("editor.range_slider", "only a slider has a minimum, a maximum and a step", &[])
    );
    let mut next = prop.as_object().cloned().unwrap_or_default();
    ranged.iter().for_each(|(k, v)| v.iter().for_each(|v| drop(next.insert((*k).into(), json!(v)))));
    e.default.iter().for_each(|d| drop(next.insert("default".into(), d.clone())));
    let mut ops = if Some(&Value::Object(next.clone())) == Some(prop) {
        Vec::new()
    } else {
        set(doc, &at, Value::Object(next))
    };
    if let Some(to) = e.order {
        ops.extend(crate::props::move_property(doc, &e.id, to, None)?);
    }
    refused(dir, doc, ops)
}
