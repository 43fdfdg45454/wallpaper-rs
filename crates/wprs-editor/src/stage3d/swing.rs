use super::gizmo::{Seen, px, seen_at, world};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Mat, Placed, Quality, Runtime, Values};

const STEPS: usize = 12;

fn ends(w: &Mat, tip: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let at = |[x, y, z]: [f32; 3]| {
        std::array::from_fn(|r| w[r * 4] * x + w[r * 4 + 1] * y + w[r * 4 + 2] * z + w[r * 4 + 3])
    };
    ([w[3], w[7], w[11]], at(tip))
}

fn turned(head: [f32; 3], tip: [f32; 3], degrees: f32) -> [f32; 3] {
    let (s, c) = degrees.to_radians().sin_cos();
    let (x, y) = (tip[0] - head[0], tip[1] - head[1]);
    [head[0] + x * c - y * s, head[1] + x * s + y * c, tip[2]]
}

fn bounded(e: &Value, kind: &str) -> Option<[f32; 2]> {
    Some([e[kind][0].as_f64()? as f32, e[kind][1].as_f64()? as f32])
}

pub(super) fn arcs(rt: &Runtime, seen: &Seen, p: &Placed, layer: &Value) -> Vec<Value> {
    let (Some(rig), Some(puppet)) = (p.model.rig.as_ref(), rt.puppet(&p.layer.id)) else { return Vec::new() };
    let worlds = puppet.joint_worlds();
    let shown = |q: [f32; 3]| seen.at(world(&p.matrix, q)).map(px);
    let mut out = Vec::new();
    for e in layer["physics"].as_array().into_iter().flatten() {
        let Some(n) = e["bone"].as_str().and_then(|b| rig.joint(b)) else { continue };
        let (head, tip) = ends(&worlds[n], rig.tip(n));
        let rotation = bounded(e, "rotation");
        let [lo, hi] = rotation.unwrap_or([-180.0, 180.0]);
        let arc: Vec<Value> = (0..=STEPS)
            .filter_map(|k| shown(turned(head, tip, -hi + (hi - lo) * k as f32 / STEPS as f32)))
            .collect();
        out.push(json!({ "bone": e["bone"], "at": shown(head), "tip": shown(tip), "rotation": rotation,
            "ends": [shown(turned(head, tip, -lo)), shown(turned(head, tip, -hi))], "arc": arc,
            "wind": e["wind"].as_f64().unwrap_or(0.0) }));
    }
    out
}

pub fn bound(dir: &Path, doc: &Value, q: &Value) -> Value {
    let (layer, name) = (q["layer"].as_str().unwrap_or_default(), q["bone"].as_str().unwrap_or_default());
    let Some(scene) = super::store::scene(dir, doc) else { return Value::Null };
    let size = scene.canvas.size.map(|v| v as f32);
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    rt.advance(q["time"].as_f64().unwrap_or(0.0).max(0.0));
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let Some(p) = frame.placed.iter().find(|p| p.layer.id == layer) else { return Value::Null };
    let (Some(rig), Some(puppet)) = (p.model.rig.as_ref(), rt.puppet(layer)) else { return Value::Null };
    let (Some(n), Some(seen)) = (rig.joint(name), seen_at(doc, &frame, p.stage)) else { return Value::Null };
    let (head, tip) = ends(&puppet.joint_worlds()[n], rig.tip(n));
    let point = |k: usize| q["point"][k].as_f64().unwrap_or(0.0) as f32;
    let best = (-180..=180)
        .map(|d| d as f32)
        .filter_map(|d| {
            let s = seen.at(world(&p.matrix, turned(head, tip, d)))?;
            Some((d, (s[0] - point(0)).powi(2) + (s[1] - point(1)).powi(2)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    json!({ "degrees": best.map(|(d, _)| -d) })
}
