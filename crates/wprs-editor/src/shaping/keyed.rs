use crate::action::KeyframeEdit;
use crate::keys::keyframe;
use crate::ops::{apply, set};
use anyhow::{Result, ensure};
use serde_json::Value;
use wprs_core::message::said;

const PARTS: [&str; 3] = ["x", "y", "base"];

pub(crate) fn write(
    work: &mut Value,
    pointer: &str,
    value: Value,
    at: Option<f64>,
    replace: bool,
) -> Result<()> {
    let stored = work.pointer(pointer).and_then(Value::as_object).cloned();
    let (Some(stored), Some(time), false) = (stored, at, replace) else {
        super::plain(work, pointer, replace)?;
        let ops = set(work, pointer, value);
        apply(work, &ops)?;
        return Ok(());
    };
    if stored.get("keys").is_some_and(Value::is_array) {
        let edit = KeyframeEdit { value: Some(value), ..KeyframeEdit::at(pointer, time) };
        let ops = keyframe(work, &edit)?;
        apply(work, &ops)?;
        return Ok(());
    }
    let parts = value.as_array().filter(|p| p.len() == 2).cloned();
    let split = stored.keys().all(|k| PARTS.contains(&k.as_str()));
    let Some(parts) = parts.filter(|_| split) else {
        return super::plain(work, pointer, false);
    };
    let free = |part: &str| work.pointer(&format!("{pointer}/{part}")).is_none_or(|v| !v.is_object());
    let keyed = |part: &str| work.pointer(&format!("{pointer}/{part}/keys")).is_some_and(Value::is_array);
    let moving = ["x", "y"].iter().all(|p| free(p) || keyed(p));
    ensure!(
        moving,
        said(
            "editor.part_follows_source",
            "a part of %1 follows another source; write over it with replace",
            &[&pointer]
        )
    );
    for (part, v) in ["x", "y"].into_iter().zip(parts) {
        write(work, &format!("{pointer}/{part}"), v, at, false)?;
    }
    Ok(())
}
