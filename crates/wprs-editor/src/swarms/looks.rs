use super::{parsed, plot, put};
use crate::ops::{Op, set};
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::{Remap, remap_table, remapped};

pub(super) fn remap(
    doc: &Value,
    sys: &str,
    index: Option<usize>,
    value: Option<&Value>,
    remove: bool,
) -> Result<Vec<Op>> {
    let at = format!("{sys}/init/remap");
    let list = doc.pointer(&at).and_then(Value::as_array).cloned().unwrap_or_default();
    let missing = || said("editor.no_remap", "the system has no remap %1", &[&index.unwrap_or(0)]);
    match (index, value, remove) {
        (Some(i), _, true) => {
            ensure!(i < list.len(), missing());
            Ok(vec![Op::Remove { path: if list.len() == 1 { at } else { format!("{at}/{i}") } }])
        }
        (i, Some(v), false) => {
            parsed::<Remap>(v, "remap")?;
            let i = i.unwrap_or(list.len());
            ensure!(i <= list.len(), missing());
            let mut next = list;
            if i == next.len() {
                next.push(v.clone())
            } else {
                next[i] = v.clone()
            }
            Ok(set(doc, &at, json!(next)))
        }
        _ => {
            bail!(said("editor.remap_what", "give a remap to add or set, or the index of one to remove", &[]))
        }
    }
}

pub(super) fn perspective(
    doc: &Value,
    sys: &str,
    focal: Option<f64>,
    (depth, speed): (Option<[f64; 2]>, Option<[f64; 2]>),
    off: bool,
) -> Vec<Op> {
    let at = |k: &str| format!("{sys}/{k}");
    if off {
        return ["perspective", "init/depth", "init/depth_speed"]
            .iter()
            .flat_map(|k| put(doc, &at(k), None))
            .collect();
    }
    let mut ops = Vec::new();
    focal.iter().for_each(|f| ops.extend(set(doc, &at("perspective"), json!({ "focal": f }))));
    if focal.is_none() && doc.pointer(&at("perspective")).is_none() {
        ops.extend(set(doc, &at("perspective"), json!({ "focal": 800 })));
    }
    depth.iter().for_each(|d| ops.extend(set(doc, &at("init/depth"), json!(d))));
    speed.iter().for_each(|d| ops.extend(set(doc, &at("init/depth_speed"), json!(d))));
    ops
}

fn holds(layer: &Value, system: &str) -> bool {
    let own = layer["type"] == "particles" && layer["system"] == system;
    own || layer["layers"].as_array().into_iter().flatten().any(|c| holds(c, system))
}

pub fn controls(doc: &Value, pointer: &str) -> Vec<Value> {
    let system =
        pointer.strip_prefix("/scene/particles/").and_then(|p| p.split('/').next()).unwrap_or_default();
    let points = doc.pointer(&format!("/scene/particles/{system}/points")).and_then(Value::as_array);
    (0..points.map_or(0, Vec::len).max(1)).map(|k| json!(format!("p{k}"))).collect()
}

pub fn sources(doc: &Value, system: &str) -> Vec<Value> {
    let drawn = |l: &Value| !matches!(l["type"].as_str(), Some("sound" | "light"));
    let layers = crate::layers::walk(doc).into_iter().map(|(_, l)| l);
    layers.filter(|l| drawn(l) && !holds(l, system)).map(|l| l["id"].clone()).collect()
}

pub(super) fn remaps(system: &Value) -> Value {
    let list = system["init"]["remap"].as_array().into_iter().flatten().enumerate();
    let rows = list.filter_map(|(k, v)| {
        let r: Remap = serde_json::from_value(v.clone()).ok()?;
        let row = remap_table(&r);
        let span = (r.from[1] - r.from[0]).abs() * 0.25;
        let (lo, hi) = (r.from[0].min(r.from[1]) - span, r.from[0].max(r.from[1]) + span);
        let curve =
            plot([f64::from(lo), f64::from(hi)], 48, |x| f64::from(remapped(x as f32, row, k as u32)));
        Some(json!({ "index": k, "remap": v, "curve": curve }))
    });
    json!(rows.collect::<Vec<_>>())
}

pub(super) fn cutout(render: &Value) -> Value {
    let Some([a, b, o]) = serde_json::from_value::<[f64; 3]>(render["cutout"].clone()).ok() else {
        return Value::Null;
    };
    let ramp = plot([0.0, 1.0], 32, |x| ((x - a) / (b - a)).clamp(0.0, 1.0) * o);
    json!({ "start": a, "end": b, "opacity": o, "ramp": ramp })
}

pub(super) fn shaders(doc: &Value, dir: &std::path::Path, render: &Value) -> Value {
    let mut scene = serde_json::from_value::<wprs_scene::Scene>(doc["scene"].clone()).ok();
    scene.iter_mut().for_each(|s| drop(crate::cache::read_files(s, dir)));
    let loaded = scene.map(|s| s.loaded).unwrap_or_default();
    let offered: Vec<&String> =
        loaded.iter().filter(|(_, d)| d.stage == wprs_scene::Stage::Particle).map(|(id, _)| id).collect();
    json!({ "current": render["shader"], "offered": offered, "warning": render.get("shader").is_some() })
}

pub(super) fn domains() -> Value {
    json!({ "daytime": [0, 1], "runtime": [0, 60], "life": [0, 30], "speed": [0, 500], "random": [0, 1],
        "distance": [0, 2000], "audio": [0, 1], "cursor": [0, 1], "size": [0, 64], "alpha": [0, 1], "color": [0, 1],
        "rotation": [-360, 360], "spin": [-720, 720], "frame": [0, 64] })
}
