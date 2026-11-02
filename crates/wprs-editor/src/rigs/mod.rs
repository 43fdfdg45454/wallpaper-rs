use crate::skin::Skin;
use serde_json::{Value, json};
use std::path::Path;

mod handles;
mod heat;
mod known;
mod outline;
mod paper;
mod pose;
mod rows;

pub(crate) use handles::rest_end;
pub use handles::{handles, wind_arrow};
pub(crate) use heat::heat_rgb;
pub use heat::{heat, heat_color, hue, vertices};
pub use known::known;
pub use outline::outline;
pub(crate) use paper::url;
pub use pose::pose;
pub(crate) use pose::{Channels, posed};
pub use rows::keys;

pub(crate) fn round(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

pub fn rig(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let clips: Vec<&Value> = layer["clips"].as_array().into_iter().flatten().map(|c| &c["id"]).collect();
    let mut out = Value::clone(&known::body(dir, doc, layer));
    out["clips"] = json!(clips);
    out["expressions"] = layer.get("expressions").cloned().unwrap_or(json!({}));
    out
}

fn body(dir: &Path, doc: &Value, layer: &Value) -> Value {
    let mut out = json!({ "bones": [], "points": [], "parts": [], "shapes": [], "defaults": [],
        "animations": [], "editable": false, "auto": false });
    let Ok((_, path, rig)) = crate::skin::rigged(dir, doc, layer) else { return out };
    out["parts"] = json!(rig.parts);
    out["shapes"] = json!(rig.shapes);
    out["defaults"] = json!(rig.defaults);
    out["animations"] =
        json!(rig.motions.iter().map(|m| json!({ "name": m.name, "length": m.length })).collect::<Vec<_>>());
    match Skin::read(dir, &path, &rig, crate::skin::size(doc, layer)) {
        Ok(skin) => {
            let worlds = skin.worlds();
            let bones = skin.bones.iter().map(|b| {
                let parent = skin.up(*b).map(|p| skin.names[p].clone());
                let at = skin.picture(skin.head(&worlds, *b)).map(round);
                json!({ "name": skin.names[*b], "parent": parent, "at": at })
            });
            out["bones"] = json!(bones.collect::<Vec<_>>());
            let points = (0..skin.names.len()).filter(|n| !skin.bones.contains(n)).map(|n| {
                let at = skin.picture(skin.head(&worlds, n)).map(round);
                let parent = skin.parents[n].map(|p| skin.names[p].clone());
                json!({ "name": skin.names[n], "at": at, "part": !skin.is_point(n), "parent": parent })
            });
            out["points"] = json!(points.collect::<Vec<_>>());
            (out["editable"], out["auto"]) = (json!(true), json!(skin.auto));
        }
        Err(_) => {
            let skin = rig.palette.iter().map(|(n, _)| &rig.joints[*n as usize]);
            let names = skin.map(|j| json!({ "name": j.name, "parent": j.parent.map(|p| rig.joints[p as usize].name.clone()) }));
            out["bones"] = json!(names.collect::<Vec<_>>());
            let loose = (0..rig.joints.len()).filter(|n| rig.palette.iter().all(|(j, _)| *j as usize != *n));
            let points = loose.map(|n| json!({ "name": rig.joints[n].name, "part": !rig.joints[n].point }));
            out["points"] = json!(points.collect::<Vec<_>>());
        }
    }
    out
}

pub fn query(dir: &Path, doc: &Value, id: &str, q: &Value) -> Value {
    match () {
        _ if q.get("weights").is_some() => heat(dir, doc, id, q["weights"].as_str()),
        _ if q.get("pose").is_some() => pose(dir, doc, id, &q["pose"]),
        _ if q.get("keys").is_some() => keys(dir, doc, id, q["keys"].as_str().unwrap_or_default()),
        _ if q.get("handles").is_some() => handles(dir, doc, id),
        _ if q.get("particles").is_some() => {
            let p = &q["particles"];
            crate::swarms::handles(dir, doc, id, p["layer"].as_str(), p["time"].as_f64().unwrap_or(0.0))
        }
        _ if q.get("budget").is_some() => {
            let b = &q["budget"];
            let quality = serde_json::from_value(b["quality"].clone()).unwrap_or_default();
            crate::swarms::budget(
                dir,
                doc,
                id,
                b["layer"].as_str(),
                b["time"].as_f64().unwrap_or(0.0),
                quality,
            )
        }
        _ if crate::lamps::asks(q) => crate::lamps::query(dir, doc, id, q),
        _ if q.get("outline").is_some() => outline(dir, doc, id),
        _ if q.get("wind").is_some() => wind_arrow(doc),
        _ => rig(dir, doc, id),
    }
}

pub fn staged(look: &Value) -> Option<Value> {
    let quality = look.get("quality").filter(|q| q.is_string());
    let restart = look.get("restart").filter(|r| r.is_u64());
    (quality.is_some() || restart.is_some()).then(|| json!({ "quality": quality, "restart": restart }))
}

pub fn preview(scene: &mut Value, look: &Value) {
    crate::lamps::preview(scene);
    let Some(id) = look["layer"].as_str() else { return };
    fn find<'a>(list: &'a mut Value, id: &str) -> Option<&'a mut Value> {
        for l in list.as_array_mut()?.iter_mut() {
            if l["id"] == id {
                return Some(l);
            }
            if let Some(found) = l.get_mut("layers").and_then(|c| find(c, id)) {
                return Some(found);
            }
        }
        None
    }
    let found = scene.get_mut("layers").and_then(|list| find(list, id));
    let Some(layer) = found.filter(|l| l["type"] == "mesh") else { return };
    if look["still"] == true
        && let Some(map) = layer.as_object_mut()
    {
        map.remove("physics");
    }
    if let Some(clip) = look["clip"].as_str() {
        layer["animations"] = json!([{ "clip": clip }]);
    }
    if let Some(shapes) = look["shapes"].as_object().filter(|s| !s.is_empty()) {
        let mut set = layer["shapes"].as_object().cloned().unwrap_or_default();
        set.extend(shapes.iter().filter(|(_, v)| v.is_number()).map(|(k, v)| (k.clone(), v.clone())));
        layer["shapes"] = json!(set);
    }
}
