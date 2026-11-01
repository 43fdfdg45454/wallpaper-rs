use super::{parsed, put};
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::{OffsetNoise, Turbulent};

fn merged(doc: &Value, at: &str, change: Option<&Value>) -> Value {
    let mut out = doc.pointer(at).filter(|v| v.is_object()).cloned().unwrap_or(json!({}));
    for (k, v) in change.and_then(Value::as_object).into_iter().flatten() {
        out[k] = v.clone();
    }
    out
}

pub(super) fn turbulent(
    doc: &Value,
    sys: &str,
    change: Option<&Value>,
    (quiet, off): (bool, bool),
) -> Result<Vec<Op>> {
    let at = format!("{sys}/init/turbulent");
    if off {
        return Ok(put(doc, &at, None));
    }
    let mut next = merged(doc, &at, change);
    if quiet {
        next.as_object_mut().into_iter().for_each(|o| drop(o.remove("audio")));
    }
    parsed::<Turbulent>(&next, "turbulent")?;
    Ok(set(doc, &at, next))
}

pub(super) fn offset(doc: &Value, sys: &str, change: Option<&Value>, off: bool) -> Result<Vec<Op>> {
    let at = format!("{sys}/init/offset_noise");
    if off {
        return Ok(put(doc, &at, None));
    }
    let next = merged(doc, &at, change);
    parsed::<OffsetNoise>(&next, "offset_noise")?;
    Ok(set(doc, &at, next))
}

pub(super) fn pace(doc: &Value, sys: &str, emitter: Option<usize>, most: Option<u32>) -> Result<Vec<Op>> {
    let count = doc.pointer(&format!("{sys}/emitters")).and_then(Value::as_array).map_or(0, Vec::len);
    let missing = emitter.unwrap_or(0);
    ensure!(
        emitter.map_or(count > 0, |e| e < count),
        said("editor.no_emitter", "the system has no emitter %1", &[&missing])
    );
    ensure!(most != Some(0), said("editor.pace", "an emitter starts at least one particle a step", &[]));
    let list = emitter.map_or(0..count, |e| e..e + 1);
    Ok(list
        .flat_map(|e| put(doc, &format!("{sys}/emitters/{e}/max_per_step"), most.map(|m| json!(m))))
        .collect())
}
