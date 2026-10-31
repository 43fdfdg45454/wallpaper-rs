use crate::actions::{EDITOR, layer};
use crate::layers::{parent, renamed, unique};
use crate::ops::{Op, escape, set};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;

fn stash(id: &str) -> String {
    format!("{EDITOR}/hidden/{}", escape(id))
}

pub fn ids(layer: &Value) -> Vec<String> {
    let mut out = vec![layer["id"].as_str().unwrap_or_default().to_owned()];
    layer["layers"].as_array().into_iter().flatten().for_each(|child| out.extend(ids(child)));
    out
}

pub fn hide(doc: &Value, id: &str, hidden: bool) -> Result<Vec<Op>> {
    let (path, value) = layer(doc, id)?;
    let field = format!("{path}/{}", if value["type"] == "sound" { "play" } else { "visible" });
    let current = doc.pointer(&field);
    Ok(match (hidden, current == Some(&json!(false))) {
        (true, false) => {
            let kept = current.filter(|v| **v != json!(true)).map(|v| set(doc, &stash(id), v.clone()));
            set(doc, &field, json!(false)).into_iter().chain(kept.into_iter().flatten()).collect()
        }
        (false, true) => match doc.pointer(&stash(id)) {
            Some(v) => vec![Op::Replace { path: field, value: v.clone() }, Op::Remove { path: stash(id) }],
            None => vec![Op::Remove { path: field }],
        },
        _ => Vec::new(),
    })
}

pub fn forget(doc: &Value, layer: &Value) -> Vec<Op> {
    ids(layer)
        .iter()
        .filter(|id| doc.pointer(&stash(id)).is_some())
        .map(|id| Op::Remove { path: stash(id) })
        .collect()
}

pub fn duplicate(doc: &Value, id: &str) -> Result<Vec<Op>> {
    let (path, value) = layer(doc, id)?;
    let (list, i) = parent(&path);
    let mut made = Vec::new();
    let copy = renamed(doc, value, &mut made);
    let kept = ids(value)
        .into_iter()
        .zip(made)
        .filter_map(|(old, new)| Some(set(doc, &stash(&new), doc.pointer(&stash(&old))?.clone())));
    let add = Op::Add { path: format!("{list}/{}", i + 1), value: copy };
    Ok(std::iter::once(add).chain(kept.flatten()).collect())
}

pub fn group(doc: &Value, ids: &[String], id: Option<&str>) -> Result<Vec<Op>> {
    ensure!(!ids.is_empty(), said("editor.name_layers_group", "name the layers to group", &[]));
    let mut found: Vec<(String, usize, Value)> = Vec::new();
    for id in ids {
        let (path, value) = layer(doc, id)?;
        let (list, i) = parent(&path);
        if !found.iter().any(|(l, j, _)| *l == list && *j == i) {
            found.push((list.to_owned(), i, value.clone()));
        }
    }
    let list = found[0].0.clone();
    ensure!(
        found.iter().all(|(l, ..)| *l == list),
        said("editor.only_layers_same", "only layers of the same group can be grouped", &[])
    );
    found.sort_by_key(|(_, i, _)| *i);
    let first = found[0].1;
    let id = id.map(String::from).unwrap_or_else(|| unique(doc, "group"));
    let layers: Vec<Value> = found.iter().map(|(.., v)| v.clone()).collect();
    let removed = found.iter().rev().map(|(l, i, _)| Op::Remove { path: format!("{l}/{i}") });
    let value = json!({"id": id, "type": "group", "layers": layers});
    Ok(removed.chain([Op::Add { path: format!("{list}/{first}"), value }]).collect())
}

pub fn ungroup(doc: &Value, id: &str) -> Result<Vec<Op>> {
    let (path, value) = layer(doc, id)?;
    ensure!(value["type"] == "group", said("editor.not_group", "%1 is not a group", &[&id]));
    let plain = json!({"id": id, "type": "group", "layers": value["layers"]});
    let normal = |v: &Value| {
        let layer = serde_json::from_value::<wprs_scene::Layer>(v.clone()).ok()?;
        serde_json::to_value(layer).ok()?.as_object().cloned()
    };
    let own = normal(value).with_context(|| {
        said("editor.group_problems", "the group %1 has problems; fix them first", &[&id])
    })?;
    let base = normal(&plain).unwrap_or_default();
    let own: Vec<&str> =
        own.iter().filter(|(k, v)| base.get(*k) != Some(*v)).map(|(k, _)| k.as_str()).collect();
    ensure!(
        own.is_empty(),
        said(
            "editor.group_own_reset",
            "the group %1 has its own %2; reset them before ungrouping",
            &[&id, &(own.join(", "))]
        )
    );
    let (list, i) = parent(&path);
    let inner = value["layers"].as_array().into_iter().flatten().enumerate();
    let added: Vec<Op> =
        inner.map(|(k, l)| Op::Add { path: format!("{list}/{}", i + k), value: l.clone() }).collect();
    Ok(std::iter::once(Op::Remove { path }).chain(added).collect())
}
