use crate::ops::Op;
use crate::skin::Skin;
use anyhow::{Result, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_core::message::said;

pub const MAX_STROKE: usize = 1024;
pub const MAX_RAISE: f32 = 1000.0;

pub(crate) fn stroke(skin: &Skin, points: &[[f32; 2]], radius: f32) -> Result<Vec<[f32; 2]>> {
    let drawn = (1..=MAX_STROKE).contains(&points.len()) && points.iter().flatten().all(|v| v.is_finite());
    ensure!(drawn, said("editor.stroke_points", "a stroke has 1 to %1 points", &[&MAX_STROKE]));
    ensure!(
        radius > 0.0 && radius <= 1.0,
        said("editor.depth_radius", "the brush has a radius above 0 and up to 1 of the longer side", &[])
    );
    Ok(points.iter().map(|p| skin.model(*p)).collect())
}

pub(crate) fn depth(
    dir: &Path,
    doc: &Value,
    layer: &str,
    points: &[[f32; 2]],
    (radius, by): (f32, f32),
) -> Result<Vec<Op>> {
    let (_, found) = crate::actions::layer(doc, layer)?;
    let (id, mut skin) = Skin::load(dir, doc, found)?;
    let stroke = stroke(&skin, points, radius)?;
    ensure!(
        by.is_finite() && by.abs() <= MAX_RAISE,
        said("editor.depth_by", "a stroke raises or lowers vertices by at most %1 pixels", &[&MAX_RAISE])
    );
    let [w, h] = skin.size;
    skin.raise(&stroke, radius * w.max(h), by);
    skin.save(dir, doc, &id)
}
