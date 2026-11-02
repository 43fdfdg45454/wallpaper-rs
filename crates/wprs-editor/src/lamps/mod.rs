use crate::actions::Action;
use crate::ops::Op;
use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

mod air;
mod cast;
mod cost;
mod data;
mod edit;
mod lit;
mod maps;
mod placed;
mod report;
mod swarm;

pub use air::{air, levels as air_levels};
pub use cast::{cast, homography};
pub use cost::cost;
pub use edit::template;
pub use lit::hemisphere;
pub use maps::{heat, preview as normals_preview};
pub use placed::{Placed, placed};
pub use report::{AIM, FEW, Z_KNOB, falloff, handles, lights, reach, tiers, tilt_share};
pub(crate) use swarm::edit as glow;
pub use swarm::{sides, swarm_light};

const QUERIES: [&str; 9] =
    ["lights", "falloff", "normals", "map", "hemisphere", "lighting", "air", "projection", "swarm_light"];

pub(crate) fn act(dir: &Path, doc: &Value, action: &Action) -> Option<Result<Vec<Op>>> {
    Some(match action {
        Action::Light(e) => edit::edit(dir, doc, e),
        Action::Lighting(e) => lit::lighting(doc, e),
        Action::Normals(m) => maps::normals(dir, doc, m),
        Action::PaintMap(s) => maps::paint(dir, doc, s),
        Action::Ambient(e) => lit::ambient(doc, e),
        _ => return None,
    })
}

pub(crate) fn moving(layer: &Value) -> bool {
    fn bound(v: &Value) -> bool {
        match v {
            Value::Object(m) => {
                m.contains_key("keys")
                    || m.contains_key("audio")
                    || m.contains_key("script")
                    || m.contains_key("pointer")
                    || m.values().any(bound)
            }
            Value::Array(l) => l.iter().any(bound),
            _ => false,
        }
    }
    bound(layer)
}

pub fn asks(q: &Value) -> bool {
    QUERIES.iter().any(|k| q.get(k).is_some())
}

pub fn query(dir: &Path, doc: &Value, id: &str, q: &Value) -> Value {
    let t = |v: &Value| v["time"].as_f64().unwrap_or(0.0);
    match () {
        _ if q.get("lights").is_some() => lights(dir, doc, t(&q["lights"])),
        _ if q.get("falloff").is_some() => {
            let f = &q["falloff"];
            falloff(dir, doc, f["layer"].as_str().unwrap_or(id), t(f))
        }
        _ if q.get("normals").is_some() => match serde_json::from_value(q["normals"].clone()) {
            Ok(m) => normals_preview(dir, doc, &m),
            Err(_) => Value::Null,
        },
        _ if q.get("map").is_some() => {
            let m = &q["map"];
            let field = if m["map"] == "emissive" { "emissive_map" } else { "roughness_map" };
            heat(dir, doc, m["layer"].as_str().unwrap_or(id), field)
        }
        _ if q.get("hemisphere").is_some() => hemisphere(doc),
        _ if q.get("air").is_some() => air(dir, doc, q["air"]["layer"].as_str().unwrap_or(id), t(&q["air"])),
        _ if q.get("projection").is_some() => {
            let p = &q["projection"];
            cast(dir, doc, p["layer"].as_str().unwrap_or(id), t(p))
        }
        _ if q.get("swarm_light").is_some() => {
            let s = &q["swarm_light"];
            swarm_light(dir, doc, s["system"].as_str().unwrap_or(id), s["layer"].as_str())
        }
        _ => {
            let l = &q["lighting"];
            cost(dir, doc, t(l), serde_json::from_value(l["quality"].clone()).unwrap_or_default())
        }
    }
}

pub fn preview(scene: &mut Value) {
    let ext = &scene["ext"]["editor"]["lights"];
    let solo = ext["solo"].as_str().map(String::from);
    let muted: Vec<String> =
        ext["muted"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
    if solo.is_none() && muted.is_empty() {
        return;
    }
    fn walk(list: &mut Value, off: &dyn Fn(&str) -> bool) {
        for l in list.as_array_mut().into_iter().flatten() {
            if l["type"] == "light" && l["id"].as_str().is_some_and(off) {
                l["visible"] = json!(false);
            }
            if let Some(inner) = l.get_mut("layers") {
                walk(inner, off);
            }
        }
    }
    let off = |id: &str| muted.iter().any(|m| m == id) || solo.as_deref().is_some_and(|s| s != id);
    if let Some(list) = scene.get_mut("layers") {
        walk(list, &off);
    }
}
