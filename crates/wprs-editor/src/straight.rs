use crate::action::KeyframeEdit;
use crate::keys::time_of;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;

fn cut(key: &Value, side: &str) -> Value {
    let plain = json!([key[0], key[1]]);
    match key.get(2) {
        Some(Value::Object(sides)) => {
            let mut sides = sides.clone();
            sides.remove(side);
            match sides.is_empty() {
                true => plain,
                false => json!([key[0], key[1], sides]),
            }
        }
        Some(_) if side == "out" => plain,
        _ => key.clone(),
    }
}

pub(crate) fn straighten(doc: &Value, e: &KeyframeEdit, time: f64) -> Result<Vec<Op>> {
    let pointer = e.pointer.as_str();
    let alone = [e.value.is_none(), e.ease.is_none(), e.in_handle.is_none(), e.out_handle.is_none()];
    let more = [!e.remove, e.from.is_none(), e.base.is_none(), !e.absolute, e.timeline.is_none()];
    ensure!(
        alone.into_iter().chain(more).all(|a| a),
        said(
            "editor.straighten_alone",
            "straighten the curve after a keyframe on its own, without other changes",
            &[]
        )
    );
    let keys = doc.pointer(pointer).and_then(|c| c.get("keys")).and_then(Value::as_array);
    let keys =
        keys.ok_or_else(|| anyhow!(said("editor.no_keyframes", "%1 has no keyframes", &[&pointer])))?;
    let i = keys.iter().position(|k| (time_of(k) - time).abs() < 1e-3).ok_or_else(|| {
        anyhow!(said("editor.no_keyframe_s", "%1 has no keyframe at %2 s", &[&pointer, &time]))
    })?;
    let mut next = keys.clone();
    next[i] = cut(&keys[i], "out");
    if let Some(after) = keys.get(i + 1) {
        next[i + 1] = cut(after, "in");
    }
    Ok(set(doc, &format!("{pointer}/keys"), Value::Array(next)))
}
