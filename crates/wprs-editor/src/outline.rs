use crate::actions::{locked, solo};
use crate::canvas::gizmos;
use crate::check::{Problem, check};
use crate::layers::{KINDS, asset_kind, walk};
use crate::ops::escape;
use crate::project::Project;
use crate::starters::STARTERS;
use serde_json::{Value, json};

fn tracks(value: &Value, pointer: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) if map.get("keys").is_some_and(Value::is_array) => {
            out.push((pointer.into(), value.clone()))
        }
        Value::Object(map) => {
            map.iter().for_each(|(k, v)| tracks(v, &format!("{pointer}/{}", escape(k)), out))
        }
        Value::Array(list) => {
            list.iter().enumerate().for_each(|(i, v)| tracks(v, &format!("{pointer}/{i}"), out))
        }
        _ => {}
    }
}

pub fn outline(project: &Project, time: f64, files: &[Problem]) -> Value {
    let doc = &project.doc;
    let (locked, solo, all) = (locked(doc), solo(doc), walk(doc));
    let layers: Vec<Value> = all
        .iter()
        .map(|(pointer, l)| {
            let id = l["id"].as_str().unwrap_or_default();
            let hidden = l["visible"] == false || l["play"] == false;
            json!({"id": id, "type": l["type"], "pointer": pointer, "depth": pointer.matches("/layers/").count() - 1,
                "hidden": hidden, "locked": locked.iter().any(|x| x == id), "solo": solo == Some(id),
                "effects": l["effects"].as_array().map_or(0, Vec::len)})
        })
        .collect();
    let layer_of = |pointer: &str| {
        let owner =
            all.iter().filter(|(p, _)| pointer.starts_with(&format!("{p}/"))).max_by_key(|(p, _)| p.len());
        owner.map_or(Value::Null, |(_, l)| l["id"].clone())
    };
    let mut found = Vec::new();
    tracks(&doc["scene"], "/scene", &mut found);
    let tracks: Vec<Value> = found
        .into_iter()
        .map(|(pointer, v)| {
            let key = pointer.rsplit('/').next().unwrap_or_default().to_owned();
            json!({"pointer": pointer, "layer": layer_of(&pointer), "key": key, "keys": v["keys"], "timeline": v["timeline"]})
        })
        .collect();
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    let assets: Vec<Value> = assets
        .map(|(id, a)| {
            let path = a["path"].as_str().unwrap_or_default();
            let size = std::fs::metadata(project.dir.join(path)).map_or(0, |m| m.len());
            json!({"id": id, "path": path, "kind": asset_kind(path), "size": size, "pointer": format!("/scene/assets/{}", escape(id))})
        })
        .collect();
    let timelines = doc["scene"]["timelines"].as_array().into_iter().flatten().enumerate();
    let timelines: Vec<Value> = timelines
        .map(|(i, t)| json!({"id": t["id"], "duration": t["duration"], "loop": t["loop"], "pointer": format!("/scene/timelines/{i}")}))
        .collect();
    let length = timelines.iter().filter_map(|t| t["duration"].as_f64()).fold(10.0, f64::max);
    let mut problems = check(doc);
    problems.extend(
        files
            .iter()
            .filter(|p| !problems.iter().any(|q| q.message == p.message))
            .cloned()
            .collect::<Vec<_>>(),
    );
    let systems: Vec<&String> =
        doc["scene"]["particles"].as_object().map(|p| p.keys().collect()).unwrap_or_default();
    let (done, undone) = project.history.labels();
    let m = &doc["manifest"];
    let title = crate::meta::title(doc, &wprs_core::locale::languages());
    json!({"title": title, "id": m["id"], "version": m["version"], "license": m["license"], "canvas": doc["scene"]["canvas"],
        "v2": m["schema"] == wprs_format::SCHEMA, "rating": m["content"]["rating"], "photosensitive": m["photosensitive"],
        "layers": layers, "gizmos": gizmos(&project.dir, doc, time), "problems": problems, "assets": assets,
        "properties": crate::listing::rows(doc), "presets": crate::listing::presets(doc),
        "property_kinds": crate::props::PROPERTY_KINDS, "timelines": timelines, "tracks": tracks, "length": length, "systems": systems,
        "history": {"done": done, "undone": undone}, "dirty": project.dirty(), "recoverable": project.pending.is_some(),
        "kinds": KINDS, "starters": STARTERS, "easings": wprs_scene::easings(),
        "effects": wprs_scene::builtin().iter().map(|d| d.id.as_str()).collect::<Vec<_>>()})
}
