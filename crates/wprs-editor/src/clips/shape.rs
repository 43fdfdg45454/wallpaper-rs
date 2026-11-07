use crate::action::ClipKey;
pub(super) use crate::handles::reach;
use crate::handles::{end, spans};
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;
use wprs_core::message::said;
use wprs_scene::Ease;

pub(super) fn check(k: &ClipKey, tracks: usize) -> Result<()> {
    if let Some(e) = &k.ease {
        serde_json::from_value::<Ease>(e.clone()).map_err(|e| anyhow!("{e}"))?;
    }
    spans(k.ease.as_ref(), [k.in_handle, k.out_handle])?;
    let handled = k.in_handle.is_some() || k.out_handle.is_some();
    ensure!(
        tracks == 1 || !handled,
        said("editor.handles_one_channel", "Bézier handles shape one channel of a key at a time", &[])
    );
    Ok(())
}

pub(super) fn third(k: &ClipKey, held: Option<Value>, value: &Value) -> Result<Option<Value>> {
    let ease = k.ease.as_ref().map(|e| (*e != "linear").then(|| e.clone()));
    match ease {
        Some(plain) => Ok(plain),
        None => end(held.as_ref(), None, [k.in_handle, k.out_handle], value),
    }
}
