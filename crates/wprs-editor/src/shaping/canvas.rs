use super::effects::put;
use super::refused;
use crate::action::CanvasEdit as C;
use crate::ops::Op;
use anyhow::{Result, bail};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;

fn tidy(v: Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => {
            let short = n.as_f64().and_then(|f| format!("{}", f as f32).parse::<f64>().ok());
            short.map_or(Value::Number(n), |f| json!(f))
        }
        Value::Object(m) => Value::Object(m.into_iter().map(|(k, v)| (k, tidy(v))).collect()),
        other => other,
    }
}

fn merged<T: serde::Serialize + Default>(doc: &Value, at: &str) -> Map<String, Value> {
    let base = serde_json::to_value(T::default())
        .map(tidy)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let mut next = base;
    doc.pointer(at)
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .for_each(|(k, v)| drop(next.insert(k.clone(), v.clone())));
    next
}

pub(crate) fn canvas(dir: &Path, doc: &Value, e: &C) -> Result<Vec<Op>> {
    let ops = match e {
        C::Bloom { off: true, .. } => put(doc, "/scene/canvas/bloom", None),
        C::Bloom { threshold, strength, radius, tint, hdr, .. } => {
            let mut next = merged::<wprs_scene::Bloom>(doc, "/scene/canvas/bloom");
            let numbers = [("threshold", threshold), ("strength", strength), ("radius", radius)];
            numbers.iter().for_each(|(k, v)| v.iter().for_each(|v| drop(next.insert((*k).into(), json!(v)))));
            tint.iter().for_each(|t| drop(next.insert("tint".into(), json!(t))));
            match hdr.as_ref().map(|h| h.as_array().map(|a| (a.first().cloned(), a.get(1).cloned()))) {
                Some(Some((Some(t), Some(s)))) => {
                    drop(next.insert("hdr".into(), json!({ "threshold": t, "strength": s })))
                }
                Some(None) if hdr.as_ref().is_some_and(|h| h.is_null() || h == "none") => {
                    drop(next.remove("hdr"))
                }
                Some(_) => bail!(said(
                    "editor.hdr_pair",
                    "the glare of a glow is none or a threshold and a strength",
                    &[]
                )),
                None => {}
            }
            put(doc, "/scene/canvas/bloom", Some(Value::Object(next)))
        }
        C::Parallax { off: true, .. } => put(doc, "/scene/canvas/parallax", None),
        C::Parallax { amount, delay, pointer, gyro, .. } => {
            let mut next = merged::<wprs_scene::Glide>(doc, "/scene/canvas/parallax");
            let numbers = [("amount", amount), ("delay", delay), ("pointer", pointer)];
            numbers.iter().for_each(|(k, v)| v.iter().for_each(|v| drop(next.insert((*k).into(), json!(v)))));
            gyro.iter().for_each(|g| drop(next.insert("gyro".into(), json!(g))));
            put(doc, "/scene/canvas/parallax", Some(Value::Object(next)))
        }
    };
    refused(dir, doc, ops)
}
