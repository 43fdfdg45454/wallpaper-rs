use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{Kind, Model, Quality, Tier, model_caps, model_verdict};

const LEVELS: [Quality; 4] = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3];

pub fn cost(dir: &Path, doc: &Value, quality: Quality) -> Value {
    let Some(scene) = super::store::scene(dir, doc) else { return Value::Null };
    let shown: Vec<(&str, &Model)> = scene
        .layers()
        .filter_map(|l| match &l.kind {
            Kind::Model(m) if l.visible != wprs_scene::Value::Fixed(false) => {
                Some((l.id.as_str(), scene.solids.get(&m.model)?.as_ref()))
            }
            _ => None,
        })
        .collect();
    let models: Vec<&Model> = shown.iter().map(|(_, m)| *m).collect();
    let bones = scene.models.values().map(|f| f.bones.len()).max().unwrap_or(0);
    let triangles: u64 = models.iter().map(|m| m.triangles).sum();
    let groups = scene
        .layers()
        .filter(|l| matches!(l.kind, Kind::Group { space: Some(wprs_scene::Space::Depth), .. }))
        .count();
    let canvas = scene.canvas.size.map(|v| v as f32);
    let levels: Vec<Value> = LEVELS
        .iter()
        .map(|q| {
            let k = q.render_scale();
            let units: f32 = models.iter().map(|m| m.triangles as f32 / 50_000.0 + 1.0).sum::<f32>() * k * k;
            json!({ "quality": q, "triangles": triangles, "pixels": round(canvas[0] * canvas[1] * k * k),
                "units": round(units), "scene": round(scene.cost(*q)), "specular": *q > Quality::Q0 })
        })
        .collect();
    let tiers: Vec<Value> = [Tier::T0, Tier::T1, Tier::T2, Tier::T3]
        .into_iter()
        .map(|t| {
            let v = model_verdict(&models, bones, t);
            let cap = model_caps(t, false);
            let memory: u64 = models.iter().map(|m| m.texture_bytes(cap.texture)).sum();
            let busy = scene.cost(t.quality(false)) > t.budget();
            json!({ "tier": t, "fits": v.fits && !busy, "reasons": v.reasons, "busy": busy, "memory": memory,
                "triangles": cap.triangles, "texture": cap.texture, "quality": t.quality(false) })
        })
        .collect();
    let list: Vec<Value> = shown
        .iter()
        .map(|(id, m)| json!({ "id": id, "triangles": m.triangles, "textures": m.textures.len() }))
        .collect();
    json!({ "models": list, "groups": groups, "levels": levels, "tiers": tiers, "quality": quality,
        "phone": model_caps(Tier::T1, true) })
}
