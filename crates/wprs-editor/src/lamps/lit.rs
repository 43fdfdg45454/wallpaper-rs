use crate::action::{AmbientEdit, LightingEdit};
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Map, Value, json};
use wprs_core::message::said;

const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

pub(crate) fn picture(doc: &Value, id: &str) -> Result<()> {
    let path = doc.pointer(&format!("/scene/assets/{}/path", escape(id))).and_then(Value::as_str);
    let ext = path.and_then(|p| p.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase());
    ensure!(
        ext.is_some_and(|e| PICTURES.contains(&e.as_str())),
        said("editor.lighting_picture", "%1 is not a picture of the scene", &[&id])
    );
    Ok(())
}

pub(crate) fn lit_layer<'a>(doc: &'a Value, id: &str) -> Result<(String, &'a Value)> {
    let (path, layer) = crate::actions::layer(doc, id)?;
    ensure!(
        matches!(layer["type"].as_str(), Some("image" | "video" | "text")),
        said("editor.lighting_kind", "lighting is for image, video and text layers", &[])
    );
    Ok((path, layer))
}

pub(crate) fn lighting(doc: &Value, e: &LightingEdit) -> Result<Vec<Op>> {
    let (path, layer) = lit_layer(doc, &e.layer)?;
    if e.off {
        return Ok(layer
            .get("lighting")
            .map(|_| Op::Remove { path: format!("{path}/lighting") })
            .into_iter()
            .collect());
    }
    let mut block: Map<String, Value> = layer["lighting"].as_object().cloned().unwrap_or_default();
    let pictures = [("normal", &e.normal), ("env", &e.env), ("roughness_map", &e.roughness_map)];
    for (name, id) in pictures.into_iter().chain([("emissive_map", &e.emissive_map)]) {
        match id.as_deref() {
            None => {}
            Some("") => drop(block.remove(name)),
            Some(id) => {
                picture(doc, id)?;
                block.insert(name.into(), json!(id));
            }
        }
    }
    let numbers = [("roughness", e.roughness, 1.0), ("reflection", e.reflection, 1.0)];
    for (name, v, hi) in
        numbers.into_iter().chain([("emissive", e.emissive, 16.0), ("strength", e.strength, 1.0)])
    {
        let Some(v) = v else { continue };
        ensure!(
            v.is_finite() && (0.0..=hi).contains(&v),
            said("editor.lighting_range", "%1 goes from 0 to %2", &[&name, &hi])
        );
        block.insert(name.into(), json!(v));
    }
    Ok(set(doc, &format!("{path}/lighting"), Value::Object(block)))
}

pub(crate) fn ambient(doc: &Value, e: &AmbientEdit) -> Result<Vec<Op>> {
    let mut ops = Vec::new();
    for (name, color) in [("ambient", &e.ambient), ("sky", &e.sky)] {
        let path = format!("/scene/canvas/{name}");
        match color {
            _ if e.clear => ops.extend(doc.pointer(&path).map(|_| Op::Remove { path: path.clone() })),
            None => {}
            Some(c) => {
                serde_json::from_value::<wprs_scene::Color>(c.clone()).map_err(|_| {
                    anyhow!(said("editor.ambient_color", "%1 is #rrggbb or [r, g, b, a]", &[&name]))
                })?;
                ops.extend(set(doc, &path, c.clone()));
            }
        }
    }
    Ok(ops)
}

fn color(doc: &Value, name: &str) -> Option<[f32; 4]> {
    serde_json::from_value::<wprs_scene::Color>(doc["scene"]["canvas"][name].clone()).ok().map(|c| c.0)
}

pub fn hemisphere(doc: &Value) -> Value {
    let ambient = color(doc, "ambient").unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let sky = color(doc, "sky").unwrap_or(ambient);
    let side = 96u32;
    let r = side as f32 / 2.0;
    let ball = image::RgbaImage::from_fn(side, side, |x, y| {
        let (u, v) = ((x as f32 + 0.5 - r) / r, (y as f32 + 0.5 - r) / r);
        if u * u + v * v > 1.0 {
            return image::Rgba([0, 0, 0, 0]);
        }
        let k = 0.5 - 0.5 * v;
        let mix =
            |i: usize| ((ambient[i] + (sky[i] - ambient[i]) * k) * 255.0).round().clamp(0.0, 255.0) as u8;
        image::Rgba([mix(0), mix(1), mix(2), 255])
    });
    json!({ "ambient": ambient, "sky": sky, "sphere": crate::rigs::url(&ball), "size": [side, side] })
}
