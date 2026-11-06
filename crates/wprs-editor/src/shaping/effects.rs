use super::refused;
use crate::action::EffectsEdit as E;
use crate::actions::layer;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Descriptor, described};

fn known(dir: &Path, doc: &Value, id: &str) -> Result<Descriptor> {
    let own = super::own(dir, doc);
    let found = own.get(id).map(|d| (**d).clone()).or_else(|| described(id).cloned());
    found.ok_or_else(|| anyhow!(said("editor.unknown_effect", "there is no effect \"%1\"", &[&id])))
}

fn bound(limit: Option<f32>, none: &str) -> String {
    limit.map_or(none.into(), |l| l.to_string())
}

fn check(d: &Descriptor, params: &Map<String, Value>) -> Result<()> {
    for (key, value) in params {
        let def = d.params.iter().find(|p| p.id == *key);
        let option = d.options.iter().any(|o| o.id == *key);
        let missing = said("editor.no_effect_param", "the effect %1 has no parameter %2", &[&d.id, key]);
        ensure!(def.is_some() || option, missing);
        let (Some(p), Some(n)) = (def, value.as_f64()) else { continue };
        let fits = p.min.is_none_or(|m| n >= f64::from(m)) && p.max.is_none_or(|m| n <= f64::from(m));
        let (lo, hi) = (bound(p.min, "-∞"), bound(p.max, "∞"));
        let range = said(
            "editor.effect_param_range",
            "%1 of the effect %2 goes from %3 to %4",
            &[key, &d.id, &lo, &hi],
        );
        ensure!(fits, range);
    }
    Ok(())
}

pub(super) fn slot(doc: &Value, id: &str, index: usize) -> Result<(String, Value)> {
    let (path, found) = layer(doc, id)?;
    let effect = found["effects"].get(index).cloned();
    let missing = said("editor.no_effect_at", "the layer %1 has no effect %2", &[&id, &index]);
    Ok((format!("{path}/effects/{index}"), effect.ok_or_else(|| anyhow!(missing))?))
}

pub(super) fn put(doc: &Value, at: &str, value: Option<Value>) -> Vec<Op> {
    match value {
        Some(v) => set(doc, at, v),
        None if doc.pointer(at).is_some() => vec![Op::Remove { path: at.into() }],
        None => Vec::new(),
    }
}

fn params(old: &Value, change: &Map<String, Value>) -> Option<Value> {
    let mut next = old.as_object().cloned().unwrap_or_default();
    for (key, value) in change {
        match value {
            Value::Null => drop(next.remove(key)),
            v => drop(next.insert(key.clone(), v.clone())),
        }
    }
    (!next.is_empty()).then_some(Value::Object(next))
}

pub(crate) fn effects(dir: &Path, doc: &Value, e: &E) -> Result<Vec<Op>> {
    match e {
        E::Add { layer: id, effect, name, at, params: p, optional } => {
            check(&known(dir, doc, effect)?, p)?;
            let (path, found) = layer(doc, id)?;
            let sound = said("editor.sound_no_effects", "a sound or light layer takes no effects", &[]);
            ensure!(!["sound", "light"].contains(&found["type"].as_str().unwrap_or_default()), sound);
            let len = found["effects"].as_array().map_or(0, Vec::len);
            let at = at.unwrap_or(len);
            ensure!(at <= len, said("editor.no_effect_at", "the layer %1 has no effect %2", &[id, &at]));
            let mut value = json!({ "effect": effect });
            name.iter().filter(|n| !n.is_empty()).for_each(|n| value["name"] = json!(n));
            params(&Value::Null, p).into_iter().for_each(|p| value["params"] = p);
            if *optional {
                value["optional"] = json!(true);
            }
            refused(dir, doc, vec![Op::Add { path: format!("{path}/effects/{at}"), value }])
        }
        E::Set { layer: id, index, params: p, enabled, name, optional } => {
            let (at, old) = slot(doc, id, *index)?;
            check(&known(dir, doc, old["effect"].as_str().unwrap_or_default())?, p)?;
            let mut ops = Vec::new();
            if !p.is_empty() {
                ops.extend(put(doc, &format!("{at}/params"), params(&old["params"], p)));
            }
            enabled.iter().for_each(|on| ops.extend(set(doc, &format!("{at}/enabled"), json!(on))));
            if let Some(n) = name {
                ops.extend(put(doc, &format!("{at}/name"), (!n.is_empty()).then(|| json!(n))));
            }
            if let Some(o) = optional {
                ops.extend(put(doc, &format!("{at}/optional"), o.then_some(json!(true))));
            }
            refused(dir, doc, ops)
        }
        E::Remove { layer: id, index } => {
            let (at, _) = slot(doc, id, *index)?;
            let list = at.rsplit_once('/').map_or(at.clone(), |(l, _)| l.to_owned());
            let alone = doc.pointer(&list).and_then(Value::as_array).is_some_and(|l| l.len() == 1);
            Ok(vec![Op::Remove { path: if alone { list } else { at } }])
        }
        E::Move { layer: id, index, to } => {
            let (at, _) = slot(doc, id, *index)?;
            let list = at.rsplit_once('/').map_or(at.clone(), |(l, _)| l.to_owned());
            let last = doc.pointer(&list).and_then(Value::as_array).map_or(0, |l| l.len() - 1);
            ensure!(*to <= last, said("editor.no_effect_at", "the layer %1 has no effect %2", &[id, to]));
            Ok(if to == index {
                Vec::new()
            } else {
                vec![Op::Move { from: at, path: format!("{list}/{to}") }]
            })
        }
        E::Mask { .. } => super::masks::mask(dir, doc, e),
    }
}
