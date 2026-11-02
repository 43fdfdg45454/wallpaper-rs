use crate::action::MaterialEdit;
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::MATERIALS;

pub const NUMBERS: [(&str, &str, f64, f64); 14] = [
    ("pbr", "metallic", 0.0, 1.0),
    ("pbr", "roughness", 0.0, 1.0),
    ("pbr", "normal_scale", 0.0, 10.0),
    ("pbr", "occlusion", 0.0, 1.0),
    ("fur", "layers", 1.0, 32.0),
    ("fur", "length", 0.0, 100.0),
    ("fur", "density", 0.0, 1.0),
    ("fur", "comb_x", -1.0, 1.0),
    ("fur", "comb_y", -1.0, 1.0),
    ("fur", "comb_z", -1.0, 1.0),
    ("foliage", "strength", 0.0, 10.0),
    ("foliage", "speed", 0.0, 10.0),
    ("chroma", "threshold", 0.0, 1.0),
    ("chroma", "softness", 0.0, 1.0),
];

fn shader_ok(doc: &Value, shader: &str) -> bool {
    MATERIALS.contains(&shader) || doc.pointer(&format!("/scene/effects/{}", escape(shader))).is_some()
}

fn color(v: &Value) -> Result<Value> {
    serde_json::from_value::<wprs_scene::Color>(v.clone()).map_err(|_| {
        anyhow!(said("editor.material_color", "a material color is #rrggbb or [r, g, b, a]", &[]))
    })?;
    Ok(v.clone())
}

pub(crate) fn material(dir: &Path, doc: &Value, e: &MaterialEdit) -> Result<Vec<Op>> {
    let (path, layer) = super::edit::model_layer(doc, &e.layer)?;
    let asset = layer["model"].as_str().unwrap_or_default();
    if let Ok((model, _)) = super::store::asset(dir, doc, asset) {
        ensure!(
            model.material(&e.name).is_some(),
            said("editor.material_name", "the model has no material named %1", &[&e.name])
        );
    }
    let at = format!("{path}/materials/{}", escape(&e.name));
    if e.clear {
        return Ok(doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect());
    }
    let mut block = doc.pointer(&at).cloned().unwrap_or_else(|| json!({}));
    if let Some(s) = &e.shader {
        ensure!(
            shader_ok(doc, s),
            said(
                "editor.material_shader",
                "%1 is not a material shader; use pbr, unlit or a package effect",
                &[&s]
            )
        );
        if block["shader"].as_str().unwrap_or("pbr") != s {
            drop(block.as_object_mut().and_then(|b| b.remove("params")));
        }
        block["shader"] = json!(s);
    }
    let flags = [
        ("lit", e.lit),
        ("double_sided", e.double_sided),
        ("cast_shadows", e.cast_shadows),
        ("receive_shadows", e.receive_shadows),
    ];
    for (name, v) in flags {
        if let Some(v) = v {
            block[name] = json!(v);
        }
    }
    for (name, v) in [("color", &e.color), ("emissive", &e.emissive)] {
        match v {
            Some(Value::Null) => drop(block.as_object_mut().and_then(|b| b.remove(name))),
            Some(v) => block[name] = color(v)?,
            None => {}
        }
    }
    let shader = block["shader"].as_str().unwrap_or("pbr").to_owned();
    for (id, v) in &e.params {
        let pbr = ["metallic", "roughness", "normal_scale", "occlusion"].contains(&id.as_str());
        let known = NUMBERS.iter().find(|(s, n, ..)| (*s == shader || pbr && *s == "pbr") && n == id);
        if let Some((.., lo, hi)) = known {
            ensure!(
                v.is_finite() && (*lo..=*hi).contains(v),
                said("editor.material_param", "%1 of a material goes from %2 to %3", &[&id, lo, hi])
            );
        } else {
            ensure!(
                !MATERIALS.contains(&shader.as_str()),
                said("editor.material_unknown", "%1 is not a parameter of %2", &[&id, &shader])
            );
        }
        block["params"][id] = json!(v);
    }
    if let Some(k) = &e.key {
        ensure!(
            shader == "chroma",
            said("editor.material_key", "only chroma materials take a key color", &[])
        );
        block["params"]["key"] = color(k)?;
    }
    Ok(set(doc, &at, block))
}

pub(crate) fn listing(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((path, layer)) = super::edit::model_layer(doc, id) else { return Value::Null };
    let Ok((model, _)) = super::store::asset(dir, doc, layer["model"].as_str().unwrap_or_default()) else {
        return Value::Null;
    };
    let shaders: Vec<String> = MATERIALS.iter().map(|m| m.to_string()).chain(packages(dir, doc)).collect();
    let params: Vec<Value> = NUMBERS[4..]
        .iter()
        .map(|(s, n, lo, hi)| json!({ "shader": s, "id": n, "min": lo, "max": hi }))
        .collect();
    let list: Vec<Value> = model
        .materials
        .iter()
        .map(|m| {
            let set = &layer["materials"][&m.name];
            let maps: Vec<Value> = wprs_scene::MAP_SLOTS
                .iter()
                .zip(m.maps)
                .map(|(slot, t)| json!({ "slot": slot, "texture": t, "thumb": t.and_then(|t| super::thumbs::thumb(&model, t)) }))
                .collect();
            json!({ "name": m.name, "pointer": format!("{path}/materials/{}", escape(&m.name)),
                "file": super::info::finish(m), "set": set, "maps": maps,
                "sphere": super::sphere::sphere(doc, m, set) })
        })
        .collect();
    json!({ "layer": id, "materials": list, "shaders": shaders, "env": layer["env"], "params": params,
        "packages": packages(dir, doc), "radius": crate::rigs::round(model.radius()) })
}

fn packages(dir: &Path, doc: &Value) -> Vec<String> {
    let effects = doc["scene"]["effects"].as_object().into_iter().flatten();
    let material = |e: &Value| {
        let text = e["descriptor"].as_str().and_then(|p| std::fs::read_to_string(dir.join(p)).ok());
        text.and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|d| d["stage"] == "material")
    };
    effects.filter(|(_, e)| material(e)).map(|(k, _)| k.clone()).collect()
}
