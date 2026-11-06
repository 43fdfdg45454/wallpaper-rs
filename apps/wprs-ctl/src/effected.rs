use crate::effecting::Effect;
use crate::steps::value;
use crate::told::{fill, told};
use serde_json::{Map, Value, json};
use wprs_editor::actions::EffectsEdit as E;

fn params(given: &[String], unset: &[String]) -> Map<String, Value> {
    let pair = |p: &String| p.split_once('=').map(|(k, v)| (k.trim().to_owned(), value(v)));
    let mut out: Map<String, Value> = given.iter().filter_map(pair).collect();
    unset.iter().for_each(|k| drop(out.insert(k.clone(), Value::Null)));
    out
}

pub(crate) fn keyed(text: &str) -> Result<String, String> {
    match text.split_once('=') {
        Some((k, _)) if !k.trim().is_empty() => Ok(text.to_owned()),
        _ => Err(fill(told("%1 is not KEY=VALUE"), &[&text])),
    }
}

pub(crate) fn edit(layer: String, what: Effect) -> E {
    match what {
        Effect::Add { effect, name, at, params: p, optional } => {
            E::Add { layer, effect, name, at, params: params(&p, &[]), optional }
        }
        Effect::Set { index, params: p, unset, enabled, name, optional } => {
            E::Set { layer, index, params: params(&p, &unset), enabled, name, optional }
        }
        Effect::Remove { index } => E::Remove { layer, index },
        Effect::Move { index, to } => E::Move { layer, index, to },
        Effect::Mask { index, texture, from, shape, channel, invert, feather, strength, off } => E::Mask {
            layer,
            index,
            texture,
            from,
            shape: shape.as_deref().map(value),
            channel: channel.and_then(|c| serde_json::from_value(json!(c)).ok()),
            invert,
            feather,
            strength,
            off,
        },
    }
}
