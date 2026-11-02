use super::placed::placed;
use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{MAX_LIGHTS, Quality};

pub fn cost(dir: &Path, doc: &Value, t: f64, quality: Quality) -> Value {
    let lights = placed(dir, doc, t).len().min(MAX_LIGHTS);
    let lit: Vec<_> = crate::canvas::gizmos(dir, doc, t)
        .into_iter()
        .filter(|g| doc.pointer(&g.pointer).is_some_and(|l| l.get("lighting").is_some()))
        .collect();
    let pixels: f64 = lit.iter().map(|g| (g.size[0] * g.scale[0] * g.size[1] * g.scale[1]).abs()).sum();
    let animated =
        crate::layers::walk(doc).into_iter().any(|(_, l)| l["type"] == "light" && super::moving(l));
    let rows = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| {
        let baked = q == Quality::Q0;
        let per_frame = if baked || !animated { 0.0 } else { pixels * lights as f64 };
        json!({ "quality": q, "baked": baked, "lights": lights, "lit_layers": lit.len(),
            "shaded": round(pixels as f32 * lights as f32), "per_frame": round(per_frame as f32) })
    });
    json!({ "quality": quality, "levels": rows, "animated": animated, "lit_layers": lit.iter().map(|g| &g.id).collect::<Vec<_>>(),
        "tiers": super::report::tiers(crate::layers::walk(doc).into_iter().filter(|(_, l)| l["type"] == "light").count()) })
}
