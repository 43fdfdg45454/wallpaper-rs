use super::plot;
use super::shapes::{Frame, target};
use serde_json::{Value, json};
use wprs_scene::{Remap, remap_table, remapped};

pub(super) fn keeps(s: &Value, f: &Frame, spots: &[Value]) -> Vec<Value> {
    let list = s["forces"].as_array().into_iter().flatten().enumerate();
    list.filter_map(|(k, o)| {
        if let Some(v) = o.get("keep_distance") {
            let (at, _) = target(&v["target"], f, spots);
            let distance = v["distance"].as_f64().unwrap_or(0.0);
            let radius = distance * f.scale();
            let ring = at.map(|c| f.round([c[0] + radius, c[1]]));
            return Some(json!({ "index": k, "kind": "keep_distance", "target": v["target"], "at": at.map(|c| f.round(c)),
                "distance": distance, "radius": radius, "ring": ring, "strength": v["strength"] }));
        }
        let v = o.get("keep_between")?;
        let ((from, _), (to, _)) = (target(&v["from"], f, spots), target(&v["to"], f, spots));
        Some(json!({ "index": k, "kind": "keep_between", "from_ref": v["from"], "to_ref": v["to"],
            "from": from.map(|c| f.round(c)), "to": to.map(|c| f.round(c)), "strength": v["strength"] }))
    })
    .collect()
}

pub(super) fn boids(s: &Value, f: &Frame) -> Value {
    let list = s["forces"].as_array().into_iter().flatten().enumerate();
    let found = list.filter_map(|(k, o)| o.get("boids").map(|b| (k, b))).next();
    found.map_or(Value::Null, |(k, b)| {
        let radius = b["radius"].as_f64().unwrap_or(0.0) * f.scale();
        json!({ "index": k, "boids": b, "radius": radius })
    })
}

pub(super) fn operators(s: &Value) -> Value {
    let list = s["forces"].as_array().into_iter().flatten().enumerate();
    let rows = list.filter_map(|(k, o)| {
        let v = o.get("remap")?;
        let r: Remap = serde_json::from_value(v.clone()).ok()?;
        let row = remap_table(&r);
        let span = (r.from[1] - r.from[0]).abs() * 0.25;
        let (lo, hi) = (r.from[0].min(r.from[1]) - span, r.from[0].max(r.from[1]) + span);
        let curve =
            plot([f64::from(lo), f64::from(hi)], 48, |x| f64::from(remapped(x as f32, row, k as u32 + 8)));
        Some(json!({ "index": k, "remap": v, "curve": curve, "window": o.get("window") }))
    });
    json!(rows.collect::<Vec<_>>())
}

pub(super) fn strands(doc: &Value, render: &Value) -> Value {
    let images = doc["scene"]["assets"].as_object().into_iter().flatten();
    let images = images
        .filter(|(_, a)| a["path"].as_str().is_some_and(|p| !p.ends_with(".webm") && !p.ends_with(".mp4")));
    let images: Vec<&String> = images.map(|(id, _)| id).collect();
    json!({ "rope": render["rope"], "trail": render["trail"], "texture": render["texture"], "textures": images })
}
