use super::{parsed, put};
use crate::ops::{Op, escape, set};
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use wprs_core::message::said;
use wprs_scene::Tunable;

const EMITTED: [&str; 7] = ["burst", "follow", "inherit", "duration", "delay", "periodic", "audio_gate"];

pub(super) fn emit(
    doc: &Value,
    sys: &str,
    emitter: usize,
    values: &Map<String, Value>,
    clear: &[String],
) -> Result<Vec<Op>> {
    let at = format!("{sys}/emitters/{emitter}");
    let mut next = doc.pointer(&at).and_then(Value::as_object).cloned().ok_or_else(|| {
        anyhow::anyhow!(said("editor.no_emitter", "the system has no emitter %1", &[&emitter]))
    })?;
    for key in values.keys().chain(clear) {
        ensure!(
            EMITTED.contains(&key.as_str()),
            said(
                "editor.no_emitter_value",
                "%1 is not a value of an emitter: %2",
                &[key, &EMITTED.join(", ")]
            )
        );
    }
    clear.iter().for_each(|k| drop(next.remove(k)));
    for (key, value) in values {
        match value {
            Value::Null | Value::Bool(false) => drop(next.remove(key)),
            v => drop(next.insert(key.clone(), v.clone())),
        }
    }
    parsed::<wprs_scene::Emitter>(&Value::Object(next.clone()), "emitter")?;
    Ok(set(doc, &at, Value::Object(next)))
}

pub(super) fn window(doc: &Value, sys: &str, index: usize, window: Option<[f64; 4]>) -> Result<Vec<Op>> {
    let at = format!("{sys}/forces/{index}");
    ensure!(doc.pointer(&at).is_some(), said("editor.no_force", "the system has no force %1", &[&index]));
    let ordered = window.is_none_or(|w| w.windows(2).all(|p| p[0] <= p[1]));
    ensure!(
        ordered,
        said("editor.window_order", "a life window rises and falls in order, each from 0 to 1", &[])
    );
    Ok(put(doc, &format!("{at}/window"), window.map(|w| json!(w))))
}

pub(super) fn lock(doc: &Value, sys: &str, lock: &[String]) -> Result<Vec<Op>> {
    let kept: Vec<String> = lock.iter().filter(|l| *l != "none").cloned().collect();
    parsed::<Vec<Tunable>>(&json!(kept), "lock")?;
    Ok(put(doc, &format!("{sys}/lock"), (!kept.is_empty()).then(|| json!(kept))))
}

fn users(value: &Value, pointer: &str, old: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("system").and_then(Value::as_str) == Some(old) {
                out.push(format!("{pointer}/system"));
            }
            map.iter().for_each(|(k, v)| users(v, &format!("{pointer}/{}", escape(k)), old, out));
        }
        Value::Array(list) => {
            list.iter().enumerate().for_each(|(i, v)| users(v, &format!("{pointer}/{i}"), old, out))
        }
        _ => {}
    }
}

pub(super) fn rename(doc: &Value, sys: &str, old: &str, to: &str) -> Result<Vec<Op>> {
    let path = format!("/scene/particles/{}", escape(to));
    ensure!(doc.pointer(&path).is_none(), said("editor.id_use", "the id %1 is in use", &[&to]));
    let mut ops = vec![Op::Move { from: sys.into(), path }];
    let mut staged = doc.clone();
    crate::ops::apply(&mut staged, &ops)?;
    let mut found = Vec::new();
    users(&staged["scene"], "/scene", old, &mut found);
    ops.extend(found.into_iter().flat_map(|at| set(&staged, &at, json!(to))));
    Ok(ops)
}
