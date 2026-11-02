use super::paper::{Paper, url};
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

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn bytes(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| v.round().clamp(0.0, 255.0) as u8)
}

pub(crate) fn heat_rgb(w: f32) -> [u8; 3] {
    let w = if w.is_finite() { w.clamp(0.0, 1.0) } else { 0.0 };
    let k = STOPS.iter().position(|s| s.0 >= w).unwrap_or(4).max(1);
    let ((a, ca), (b, cb)) = (STOPS[k - 1], STOPS[k]);
    let f = (w - a) / (b - a);
    bytes(std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * f))
}

pub fn heat_color(w: f32) -> String {
    hex(heat_rgb(w))
}

fn hue_rgb(i: usize) -> [u8; 3] {
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
    bytes([r, g, b].map(|c: f32| (0.35 + 0.6 * c) * 255.0))
}

pub fn hue(i: usize) -> String {
    hex(hue_rgb(i))
}

fn owner(skin: &Skin, t: &[u32; 3]) -> [u8; 3] {
    let mut sum: Vec<(usize, f32)> = Vec::new();
    for (b, x) in t.iter().flat_map(|v| &skin.weights[*v as usize]) {
        match sum.iter_mut().find(|s| s.0 == *b) {
            Some(s) => s.1 += x,
            None => sum.push((*b, *x)),
        }
    }
    let top = sum.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|s| s.0);
    top.and_then(|n| skin.bones.iter().position(|b| *b == n)).map_or([128; 3], hue_rgb)
}

pub fn heat(dir: &Path, doc: &Value, id: &str, bone: Option<&str>) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, mut skin)) = Skin::load(dir, doc, layer) else { return Value::Null };
    skin.bind(skin.auto);
    let picked = bone.and_then(|b| skin.bone(b).ok());
    let weights: Vec<f32> =
        (0..skin.points.len()).map(|v| picked.map_or(0.0, |b| skin.weight(v, b))).collect();
    let mut paper = Paper::new(&skin);
    for t in &skin.triangles {
        let flat = owner(&skin, t);
        paper.fill(
            &skin,
            t,
            |w| {
                heat_rgb(
                    w[0] * weights[t[0] as usize]
                        + w[1] * weights[t[1] as usize]
                        + w[2] * weights[t[2] as usize],
                )
            },
            flat,
        );
    }
    let bones: Vec<&String> = skin.bones.iter().map(|b| &skin.names[*b]).collect();
    let colors: Vec<String> = (0..bones.len()).map(hue).collect();
    json!({ "bone": bone, "bones": bones, "colors": colors, "auto": skin.auto,
        "size": [paper.heat.width(), paper.heat.height()], "heat": url(&paper.heat), "owner": url(&paper.owner) })
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
