use super::effects::{put, slot};
use super::refused;
use crate::action::EffectsEdit as E;
use crate::ops::Op;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

const SOURCES: [&str; 3] = ["texture", "layer", "shape"];

pub(super) fn mask(dir: &Path, doc: &Value, e: &E) -> Result<Vec<Op>> {
    let E::Mask { layer: id, index, texture, from, shape, channel, invert, feather, strength, off } = e
    else {
        return Ok(Vec::new());
    };
    let (at, old) = slot(doc, id, *index)?;
    let at = format!("{at}/mask");
    if *off {
        return Ok(put(doc, &at, None));
    }
    let mut mask = old["mask"].as_object().cloned().unwrap_or_default();
    let given = [texture.as_ref().map(|t| json!(t)), from.as_ref().map(|f| json!(f)), shape.clone()];
    let sources = given.iter().filter(|g| g.is_some()).count();
    ensure!(
        sources <= 1,
        said("editor.mask_one_source", "a mask takes one of a picture, a layer or a shape", &[])
    );
    if sources == 1 {
        SOURCES.iter().for_each(|k| drop(mask.remove(*k)));
    }
    for (key, value) in SOURCES.iter().zip(given) {
        value.into_iter().for_each(|v| drop(mask.insert((*key).into(), v)));
    }
    channel.iter().for_each(|c| drop(mask.insert("channel".into(), json!(c))));
    invert.iter().for_each(|i| drop(mask.insert("invert".into(), json!(i))));
    feather.iter().for_each(|f| drop(mask.insert("feather".into(), json!(f))));
    strength.iter().for_each(|s| drop(mask.insert("strength".into(), json!(s))));
    let sourced = SOURCES.iter().any(|k| mask.contains_key(*k));
    ensure!(sourced, said("editor.mask_source", "a mask needs a picture, a layer or a shape", &[]));
    refused(dir, doc, put(doc, &at, Some(Value::Object(mask))))
}
