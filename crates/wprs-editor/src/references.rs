use crate::ops::{Op, diff, escape, set};
use crate::props::{clash, path, valid};
use crate::settings::{condition as problem, parse};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_scene::Condition;

pub fn rename_property(doc: &Value, id: &str, to: &str) -> Result<Vec<Op>> {
    ensure!(doc.pointer(&path(id)).is_some(), "there is no property {id:?}");
    valid(to)?;
    ensure!(!clash(doc, to, id), "the id {to} is in use");
    let renames = wprs_format::Renames::from([(id.to_owned(), to.to_owned())]);
    let (mut manifest, mut scene) = (doc["manifest"].clone(), doc["scene"].clone());
    wprs_format::rename_json(&mut manifest, Some(&mut scene), &renames);
    let mut ops = diff(&doc["manifest"], &manifest, "/manifest");
    ops.extend(diff(&doc["scene"], &scene, "/scene"));
    Ok(ops)
}

pub fn remove_property(doc: &Value, id: &str) -> Result<Vec<Op>> {
    ensure!(doc.pointer(&path(id)).is_some(), "there is no property {id:?}");
    let users = users(doc, id);
    ensure!(users.is_empty(), "{id} is still named by {}; change them first", users.join(", "));
    let mut manifest = doc["manifest"].clone();
    let props = manifest["properties"].as_object_mut().into_iter().flat_map(|all| {
        all.remove(id);
        all.values_mut().filter_map(Value::as_object_mut)
    });
    for prop in props {
        if prop.get("group").is_some_and(|g| g == id) {
            prop.remove("group");
        }
        for key in ["visible_if", "enabled_if"] {
            if let Some(Value::Object(map)) = prop.get_mut(key) {
                map.remove(id);
                if map.is_empty() {
                    prop.remove(key);
                }
            }
        }
    }
    let presets = manifest.get_mut("presets").and_then(Value::as_array_mut).into_iter().flatten();
    for values in presets.filter_map(|p| p.get_mut("values").and_then(Value::as_object_mut)) {
        values.remove(id);
    }
    Ok(diff(&doc["manifest"], &manifest, "/manifest"))
}

fn users(doc: &Value, id: &str) -> Vec<String> {
    let all = doc["manifest"]["properties"].as_object().into_iter().flatten();
    let mut out: Vec<String> = all
        .filter(|(other, _)| *other != id)
        .flat_map(|(other, p)| ["visible_if", "enabled_if"].map(move |key| (other, key, &p[key])))
        .filter(|(_, _, c)| {
            c.as_str().is_some_and(|t| Condition::Expr(t.into()).ids().iter().any(|n| n == id))
        })
        .map(|(other, key, _)| format!("{}/{key}", path(other)))
        .collect();
    bound(&doc["scene"], "/scene", id, &mut out);
    out
}

fn bound(value: &Value, pointer: &str, id: &str, out: &mut Vec<String>) {
    let trigger = format!("prop:{id}");
    match value {
        Value::Object(map) if map.get("prop").is_some_and(|p| p == id) => out.push(pointer.into()),
        Value::Object(map) => map.iter().for_each(|(key, v)| {
            let here = format!("{pointer}/{}", escape(key));
            match key == "trigger" && v.as_str() == Some(&trigger) {
                true => out.push(here),
                false => bound(v, &here, id, out),
            }
        }),
        Value::Array(list) => {
            list.iter().enumerate().for_each(|(i, v)| bound(v, &format!("{pointer}/{i}"), id, out))
        }
        _ => {}
    }
}

pub fn scripts(dir: &Path, doc: &Value, id: &str) -> Vec<String> {
    let quoted = ['"', '\'', '`'].map(|q| format!("{q}{id}{q}"));
    let list = doc["scene"]["scripts"].as_array().into_iter().flatten().filter_map(|s| s["path"].as_str());
    let reads = |path: &&str| {
        let text =
            wprs_format::check_path(path).ok().and_then(|_| std::fs::read_to_string(dir.join(path)).ok());
        text.is_some_and(|text| quoted.iter().any(|q| text.contains(q)))
    };
    list.filter(reads).map(String::from).collect()
}

pub fn condition(doc: &Value, id: &str, enabled: bool, text: &str) -> Result<Vec<Op>> {
    ensure!(doc.pointer(&path(id)).is_some(), "there is no property {id:?}");
    let key = if enabled { "enabled_if" } else { "visible_if" };
    let pointer = format!("{}/{key}", path(id));
    if let Some(reason) = problem(doc, &pointer, text) {
        bail!("{id}: {key} {reason}");
    }
    Ok(match parse(text) {
        c if c.is_empty() => {
            doc.pointer(&pointer).map(|_| Op::Remove { path: pointer.clone() }).into_iter().collect()
        }
        c => set(doc, &pointer, serde_json::to_value(c)?),
    })
}
