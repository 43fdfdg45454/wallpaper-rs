mod effects;
mod masks;

use crate::check::review;
use crate::ops::{Op, apply};
use anyhow::{Result, bail};
use serde_json::Value;
use std::path::Path;

pub(crate) use effects::effects;

pub(crate) fn own(dir: &Path, doc: &Value) -> crate::inspect::Own {
    let Ok(mut scene) = serde_json::from_value::<wprs_scene::Scene>(doc["scene"].clone()) else {
        return Default::default();
    };
    crate::cache::read_files(&mut scene, dir);
    scene.loaded
}

pub(crate) fn refused(dir: &Path, doc: &Value, ops: Vec<Op>) -> Result<Vec<Op>> {
    let mut probe = doc.clone();
    apply(&mut probe, &ops)?;
    let before: Vec<String> = review(doc, Some(dir)).into_iter().map(|p| p.message).collect();
    let fresh = review(&probe, Some(dir)).into_iter().find(|p| !p.warning && !before.contains(&p.message));
    if let Some(problem) = fresh {
        bail!(problem.said);
    }
    Ok(ops)
}
