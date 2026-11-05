use super::effects::put;
use super::refused;
use crate::action::LayerEdit as L;
use crate::actions::layer;
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;

const SIZED: [&str; 5] = ["image", "video", "solid", "adjustment", "spectrum"];
const TUNABLE: [&str; 9] =
    ["alpha", "size", "count", "speed", "life", "time_scale", "brightness", "color", "points"];

type Fields<'a> = [(&'a str, Option<Value>); 5];

fn transform(doc: &Value, path: &str, found: &Value, fields: Fields, replace: bool) -> Result<Vec<Op>> {
    let at = format!("{path}/transform");
    let mut next = found["transform"].as_object().cloned().unwrap_or_default();
    let given: Vec<(&str, Value)> = fields.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
    if given.is_empty() {
        return Ok(Vec::new());
    }
    for (k, v) in given {
        super::plain(doc, &format!("{at}/{k}"), replace)?;
        next.insert(k.into(), v);
    }
    Ok(set(doc, &at, Value::Object(next)))
}

fn instance(
    doc: &Value,
    path: &str,
    found: &Value,
    values: &Map<String, Value>,
    clear: bool,
) -> Result<Vec<Op>> {
    let kind = found["type"].as_str().unwrap_or_default();
    ensure!(
        kind == "particles",
        said("editor.instance_particles", "only a particles layer has an instance", &[])
    );
    for key in values.keys() {
        let known = TUNABLE.contains(&key.as_str());
        ensure!(
            known,
            said("editor.no_tunable", "%1 is not a value of an instance: %2", &[key, &TUNABLE.join(", ")])
        );
    }
    let at = format!("{path}/instance");
    let mut next =
        if clear { Map::new() } else { found["instance"].as_object().cloned().unwrap_or_default() };
    for (key, value) in values {
        match value {
            Value::Null => drop(next.remove(key)),
            v => drop(next.insert(key.clone(), v.clone())),
        }
    }
    Ok(put(doc, &at, (!next.is_empty()).then_some(Value::Object(next))))
}

pub(crate) fn edit(dir: &Path, doc: &Value, e: &L) -> Result<Vec<Op>> {
    let ops = match e {
        L::Transform { layer: id, position, rotation, scale, skew, origin, size, replace } => {
            let (path, found) = layer(doc, id)?;
            let turn = rotation.is_none_or(|r| (-360.0..=360.0).contains(&r));
            ensure!(turn, said("editor.rotation_range", "the rotation goes from -360 to 360 degrees", &[]));
            let sized = size.is_none_or(|s| s.iter().all(|n| *n > 0.0));
            ensure!(sized, said("editor.size_positive", "a size is above 0 pixels", &[]));
            let pair = |v: &Option<[f64; 2]>| v.map(|v| json!(v));
            let fields = [
                ("position", pair(position)),
                ("rotation", rotation.map(|r| json!(r))),
                ("scale", pair(scale)),
                ("skew", pair(skew)),
                ("origin", pair(origin)),
            ];
            let mut ops = transform(doc, &path, found, fields, *replace)?;
            if let Some(s) = size {
                let kind = found["type"].as_str().unwrap_or_default();
                ensure!(
                    SIZED.contains(&kind),
                    said("editor.no_layer_size", "a %1 layer takes no size", &[&kind])
                );
                ops.extend(set(doc, &format!("{path}/size"), json!(s)));
            }
            ops
        }
        L::Blend { layer: id, blend } => {
            let (path, _) = layer(doc, id)?;
            set(doc, &format!("{path}/blend"), json!(blend))
        }
        L::Instance { layer: id, values, clear } => {
            let (path, found) = layer(doc, id)?;
            instance(doc, &path, found, values, *clear)?
        }
    };
    refused(dir, doc, ops)
}
