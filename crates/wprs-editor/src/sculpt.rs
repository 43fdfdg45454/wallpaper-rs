use crate::ops::{Op, escape, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

pub const MAX_STROKE: usize = 1024;
pub const MAX_RAISE: f32 = 1000.0;

pub(crate) fn depth(
    dir: &Path,
    doc: &Value,
    layer: &str,
    points: &[[f32; 2]],
    (radius, by): (f32, f32),
) -> Result<Vec<Op>> {
    let (_, found) = crate::actions::layer(doc, layer)?;
    let (id, mut sketch) = crate::rig::sketch_of(dir, doc, found)?;
    let drawn = (1..=MAX_STROKE).contains(&points.len()) && points.iter().flatten().all(|v| v.is_finite());
    ensure!(drawn, said("editor.depth_stroke", "a depth stroke has 1 to %1 points", &[&MAX_STROKE]));
    ensure!(
        radius > 0.0 && radius <= 1.0,
        said("editor.depth_radius", "the brush has a radius above 0 and up to 1 of the longer side", &[])
    );
    ensure!(
        by.is_finite() && by.abs() <= MAX_RAISE,
        said("editor.depth_by", "a stroke raises or lowers vertices by at most %1 pixels", &[&MAX_RAISE])
    );
    let [w, h] = sketch.size;
    let stroke: Vec<[f32; 2]> = points.iter().map(|p| [p[0] * w, p[1] * h]).collect();
    sketch.raise(&stroke, radius * w.max(h), by);
    let saved = crate::rig::store(dir, &id, &sketch.glb())?;
    Ok(set(doc, &format!("/scene/assets/{}/path", escape(&id)), json!(saved)))
}
