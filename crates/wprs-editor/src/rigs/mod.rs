use crate::skin::Skin;
use serde_json::{Value, json};
use std::path::Path;

mod heat;
mod pose;
mod rows;

pub use heat::{heat, heat_color, hue, vertices};
pub use pose::pose;
pub use rows::keys;

pub(crate) fn round(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

pub fn rig(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let clips: Vec<&Value> = layer["clips"].as_array().into_iter().flatten().map(|c| &c["id"]).collect();
    let expressions = layer.get("expressions").cloned().unwrap_or(json!({}));
    let mut out = json!({ "clips": clips, "bones": [], "parts": [], "shapes": [], "defaults": [],
        "expressions": expressions, "animations": [], "editable": false, "auto": false });
    let Ok((_, _, rig)) = crate::skin::rigged(dir, doc, layer) else { return out };
    out["parts"] = json!(rig.parts);
    out["shapes"] = json!(rig.shapes);
    out["defaults"] = json!(rig.defaults);
    out["animations"] =
        json!(rig.motions.iter().map(|m| json!({ "name": m.name, "length": m.length })).collect::<Vec<_>>());
    match Skin::load(dir, doc, layer) {
        Ok((_, skin)) => {
            let worlds = skin.worlds();
            let bones = skin.bones.iter().map(|b| {
                let parent = skin.up(*b).map(|p| skin.names[p].clone());
                let at = skin.picture(skin.head(&worlds, *b)).map(round);
                json!({ "name": skin.names[*b], "parent": parent, "at": at })
            });
            out["bones"] = json!(bones.collect::<Vec<_>>());
            (out["editable"], out["auto"]) = (json!(true), json!(skin.auto));
        }
        Err(_) => {
            let skin = rig.palette.iter().map(|(n, _)| &rig.joints[*n as usize]);
            let names = skin.map(|j| json!({ "name": j.name, "parent": j.parent.map(|p| rig.joints[p as usize].name.clone()) }));
            out["bones"] = json!(names.collect::<Vec<_>>());
        }
    }
    out
}

pub fn query(dir: &Path, doc: &Value, id: &str, q: &Value) -> Value {
    match () {
        _ if q.get("weights").is_some() => heat(dir, doc, id, q["weights"].as_str()),
        _ if q.get("pose").is_some() => pose(dir, doc, id, &q["pose"]),
        _ if q.get("keys").is_some() => keys(dir, doc, id, q["keys"].as_str().unwrap_or_default()),
        _ => rig(dir, doc, id),
    }
}

pub fn preview(scene: &mut Value, look: &Value) {
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
    if let Some(clip) = look["clip"].as_str() {
        layer["animations"] = json!([{ "clip": clip }]);
    }
    if let Some(shapes) = look["shapes"].as_object().filter(|s| !s.is_empty()) {
        let mut set = layer["shapes"].as_object().cloned().unwrap_or_default();
        set.extend(shapes.iter().filter(|(_, v)| v.is_number()).map(|(k, v)| (k.clone(), v.clone())));
        layer["shapes"] = json!(set);
    }
}
