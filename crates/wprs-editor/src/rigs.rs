use crate::ops::escape;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::Rigged;

pub fn rig(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let clips: Vec<&Value> = layer["clips"].as_array().into_iter().flatten().map(|c| &c["id"]).collect();
    let mut out = json!({ "clips": clips, "bones": [], "parts": [], "shapes": [], "animations": [], "editable": false });
    let path = layer["mesh"]
        .as_str()
        .and_then(|m| doc.pointer(&format!("/scene/assets/{}/path", escape(m)))?.as_str());
    let Some(rig) = path.and_then(|p| Rigged::decode(&std::fs::read(dir.join(p)).ok()?).ok()) else {
        return out;
    };
    out["parts"] = json!(rig.parts);
    out["shapes"] = json!(rig.shapes);
    out["animations"] =
        json!(rig.motions.iter().map(|m| json!({ "name": m.name, "length": m.length })).collect::<Vec<_>>());
    match crate::rig::sketch_of(dir, doc, layer) {
        Ok((_, sketch)) => {
            let bones = sketch.bones.iter().map(|b| {
                let parent = b.parent.map(|p| sketch.bones[p].name.clone());
                let at = [0, 1].map(|k| (f64::from(b.at[k] / sketch.size[k]) * 1e4).round() / 1e4);
                json!({ "name": b.name, "parent": parent, "at": at })
            });
            out["bones"] = json!(bones.collect::<Vec<_>>());
            out["editable"] = json!(true);
        }
        Err(_) => {
            let skin = rig.palette.iter().map(|(n, _)| &rig.joints[*n as usize]);
            let names = skin.map(|j| json!({ "name": j.name, "parent": j.parent.map(|p| rig.joints[p as usize].name.clone()) }));
            out["bones"] = json!(names.collect::<Vec<_>>());
        }
    }
    out
}
