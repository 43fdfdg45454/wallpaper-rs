pub(crate) mod canvas;
mod effects;
mod keyed;
pub(crate) mod layers;
pub(crate) mod marks;
mod masks;
mod page;

use crate::check::review;
use crate::ops::{Op, apply};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_core::message::said;

pub(crate) use effects::effects;

pub(crate) fn own(dir: &Path, doc: &Value) -> crate::inspect::Own {
    let Ok(mut scene) = serde_json::from_value::<wprs_scene::Scene>(doc["scene"].clone()) else {
        return Default::default();
    };
    crate::cache::read_files(&mut scene, dir);
    scene.loaded
}

pub(crate) fn refused(dir: &Path, doc: &Value, ops: Vec<Op>) -> Result<Vec<Op>> {
    checked(Some(dir), doc, ops)
}

pub(crate) fn checked(dir: Option<&Path>, doc: &Value, ops: Vec<Op>) -> Result<Vec<Op>> {
    let mut probe = doc.clone();
    apply(&mut probe, &ops)?;
    let before: Vec<String> = review(doc, dir).into_iter().map(|p| p.message).collect();
    let fresh = review(&probe, dir).into_iter().find(|p| !p.warning && !before.contains(&p.message));
    if let Some(problem) = fresh {
        bail!(problem.said);
    }
    Ok(ops)
}

pub(crate) fn plain(doc: &Value, pointer: &str, replace: bool) -> Result<()> {
    let driven = doc.pointer(pointer).is_some_and(Value::is_object);
    ensure!(
        replace || !driven,
        said(
            "editor.follows_another_source",
            "%1 follows another source; set a plain value first",
            &[&pointer]
        )
    );
    Ok(())
}

pub(crate) fn sheet(
    dir: &Path,
    doc: &Value,
    mut ops: Vec<Op>,
    linear: bool,
    atlas: Option<[u32; 2]>,
) -> Result<Vec<Op>> {
    if !linear && atlas.is_none() {
        return Ok(ops);
    }
    let fits = atlas.is_none_or(|a| a.iter().all(|n| (1..=256).contains(n)));
    ensure!(
        fits,
        said("editor.atlas_range", "a sprite sheet has from 1 to 256 rows and from 1 to 256 columns", &[])
    );
    if let Some(Op::Add { value, .. } | Op::Replace { value, .. }) = ops.last_mut() {
        if linear {
            value["linear"] = serde_json::json!(true);
        }
        atlas.iter().for_each(|a| value["atlas"] = serde_json::json!(a));
    }
    refused(dir, doc, ops)
}
