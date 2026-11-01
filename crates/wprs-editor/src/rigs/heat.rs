use super::round;
use crate::skin::Skin;
use serde_json::{Map, Value, json};
use std::path::Path;

const STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [30.0, 40.0, 120.0]),
    (0.25, [30.0, 140.0, 210.0]),
    (0.5, [50.0, 180.0, 80.0]),
    (0.75, [240.0, 200.0, 40.0]),
    (1.0, [220.0, 40.0, 40.0]),
];

fn hex(c: [f32; 3]) -> String {
    let [r, g, b] = c.map(|v| v.round().clamp(0.0, 255.0) as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn heat_color(w: f32) -> String {
    let w = w.clamp(0.0, 1.0);
    let k = STOPS.iter().position(|s| s.0 >= w).unwrap_or(4).max(1);
    let ((a, ca), (b, cb)) = (STOPS[k - 1], STOPS[k]);
    let f = (w - a) / (b - a);
    hex(std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * f))
}

pub fn hue(i: usize) -> String {
    let h = (i as f32 * 137.508) % 360.0 / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let [r, g, b] = match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    };
    hex([r, g, b].map(|c: f32| (0.35 + 0.6 * c) * 255.0))
}

pub fn heat(dir: &Path, doc: &Value, id: &str, bone: Option<&str>) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, mut skin)) = Skin::load(dir, doc, layer) else { return Value::Null };
    skin.bind(skin.auto);
    let picked = bone.and_then(|b| skin.bone(b).ok());
    let points: Vec<[f64; 2]> = skin.points.iter().map(|p| skin.picture(*p).map(round)).collect();
    let mut heat = Vec::with_capacity(skin.triangles.len());
    let mut owner = Vec::with_capacity(skin.triangles.len());
    for t in &skin.triangles {
        let w = picked.map_or(0.0, |b| t.iter().map(|v| skin.weight(*v as usize, b)).sum::<f32>() / 3.0);
        heat.push(heat_color(w));
        let mut sum: Vec<(usize, f32)> = Vec::new();
        for (b, x) in t.iter().flat_map(|v| &skin.weights[*v as usize]) {
            match sum.iter_mut().find(|s| s.0 == *b) {
                Some(s) => s.1 += x,
                None => sum.push((*b, *x)),
            }
        }
        let top = sum.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|s| s.0);
        let at = top.and_then(|n| skin.bones.iter().position(|b| *b == n));
        owner.push(at.map_or("#808080".to_owned(), hue));
    }
    let bones: Vec<&String> = skin.bones.iter().map(|b| &skin.names[*b]).collect();
    let colors: Vec<String> = (0..bones.len()).map(hue).collect();
    json!({ "bone": bone, "bones": bones, "colors": colors, "points": points,
        "triangles": skin.triangles, "heat": heat, "owner": owner, "auto": skin.auto })
}

pub fn vertices(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, mut skin)) = Skin::load(dir, doc, layer) else { return Value::Null };
    skin.bind(skin.auto);
    let parts: Vec<String> = skin.glb.json["meshes"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, m)| m["name"].as_str().filter(|n| !n.is_empty()).map_or(format!("mesh_{i}"), String::from))
        .collect();
    let mut out = Vec::with_capacity(skin.points.len());
    for p in &skin.pieces {
        for v in p.first..p.first + p.count {
            let weights: Map<String, Value> =
                skin.weights[v].iter().map(|(b, w)| (skin.names[*b].clone(), json!(round(*w)))).collect();
            let at = skin.picture(skin.points[v]).map(round);
            out.push(json!({ "at": at, "part": parts.get(p.mesh), "weights": weights }));
        }
    }
    Value::Array(out)
}
