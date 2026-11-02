use super::lit::picture;
use crate::action::SwarmLight;
use crate::ops::{Op, escape};
use crate::swarms::{checked, put, render, system};
use anyhow::{Result, ensure};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Quality, Tier};

const SIDE: u32 = 48;

fn named(doc: &Value, id: &str) -> Result<Option<Value>> {
    if id.is_empty() {
        return Ok(None);
    }
    picture(doc, id)?;
    Ok(Some(json!(id)))
}

pub(crate) fn edit(dir: &Path, doc: &Value, e: &SwarmLight) -> Result<Vec<Op>> {
    let ops = match e {
        SwarmLight::Lit { system: id, layer, lit, normal } => {
            let normal = normal.as_deref().map(|n| named(doc, n)).transpose()?;
            render(doc, &system(doc, id)?, layer.as_deref(), |doc, at| {
                let mut ops = Vec::new();
                lit.iter()
                    .for_each(|on| ops.extend(put(doc, &format!("{at}/lit"), on.then_some(json!(true)))));
                normal.iter().for_each(|n| ops.extend(put(doc, &format!("{at}/normal"), n.clone())));
                ops
            })?
        }
        SwarmLight::Refract { system: id, layer, normal, amount, off } => {
            let sys = system(doc, id)?;
            if *off {
                let ops =
                    render(doc, &sys, layer.as_deref(), |doc, at| put(doc, &format!("{at}/refract"), None))?;
                return checked(dir, doc, ops);
            }
            let was = rendered(doc, id, layer.as_deref())["refract"].clone();
            let normal = match normal.as_deref() {
                Some(n) => named(doc, n)?,
                None => was.get("normal").cloned(),
            };
            let Some(normal) = normal else {
                anyhow::bail!(said("editor.refract_normal", "refraction needs a normal map", &[]));
            };
            let amount = amount.or_else(|| was["amount"].as_f64()).unwrap_or(0.3);
            let fits = amount.is_finite() && (0.0..=1.0).contains(&amount);
            ensure!(fits, said("editor.refract_amount", "the amount of refraction goes from 0 to 1", &[]));
            let next = json!({ "normal": normal, "amount": amount });
            render(doc, &sys, layer.as_deref(), |doc, at| {
                put(doc, &format!("{at}/refract"), Some(next.clone()))
            })?
        }
    };
    checked(dir, doc, ops)
}

fn rendered<'a>(doc: &'a Value, id: &str, layer: Option<&str>) -> &'a Value {
    let s = doc.pointer(&format!("/scene/particles/{}", escape(id))).unwrap_or(&Value::Null);
    let own = layer.and_then(|l| crate::actions::layer(doc, l).ok()).map(|(_, l)| &l["render"]);
    own.filter(|r| r.is_object()).unwrap_or(&s["render"])
}

fn pixels(dir: &Path, doc: &Value, id: Option<&str>) -> Option<RgbaImage> {
    let path = doc.pointer(&format!("/scene/assets/{}/path", escape(id?))).and_then(Value::as_str)?;
    let image = image::open(dir.join(path)).ok()?;
    Some(image::imageops::resize(&image.into_rgba8(), SIDE, SIDE, image::imageops::FilterType::Triangle))
}

pub fn sides(normal: Option<&RgbaImage>) -> RgbaImage {
    let r = SIDE as f32 / 2.0;
    RgbaImage::from_fn(SIDE * 2, SIDE, |x, y| {
        let (back, x) = (x >= SIDE, x % SIDE);
        let (u, v) = ((x as f32 + 0.5 - r) / r, (y as f32 + 0.5 - r) / r);
        if u * u + v * v > 1.0 {
            return Rgba([0, 0, 0, 0]);
        }
        let n = normal.map_or([0.0, 0.0, 1.0], |m| {
            let p = m.get_pixel(x, y).0.map(|c| f32::from(c) / 127.5 - 1.0);
            [p[0], p[1], p[2]]
        });
        let lz = if back { -1.2 } else { 1.2 };
        let l = [-1.0f32, -1.0, lz];
        let len = l.iter().map(|c| c * c).sum::<f32>().sqrt();
        let facing = [n[0], -n[1], if back { -n[2] } else { n[2] }];
        let ndl = (facing.iter().zip(l).map(|(a, b)| a * b).sum::<f32>() / len).max(0.0);
        let g = (40.0 + 215.0 * ndl).round() as u8;
        Rgba([g, g, g, 255])
    })
}

fn levels(lit: bool, refract: bool) -> Value {
    let rows = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| {
        json!({ "quality": q, "lights": if lit { super::FEW } else { 0 }, "refract": q.bend().filter(|_| refract) })
    });
    json!(rows)
}

pub fn swarm_light(dir: &Path, doc: &Value, id: &str, layer: Option<&str>) -> Value {
    let Some(s) = doc.pointer(&format!("/scene/particles/{}", escape(id))) else { return Value::Null };
    let r = rendered(doc, id, layer);
    let (lit, bend) = (r["lit"] == true, &r["refract"]);
    let pictures: Vec<&String> = doc["scene"]["assets"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| picture(doc, k).is_ok())
        .map(|(k, _)| k)
        .collect();
    let size =
        s["init"]["size"].as_array().map_or(16.0, |l| l.iter().filter_map(Value::as_f64).sum::<f64>() / 2.0);
    let shift = bend["amount"].as_f64().unwrap_or(0.0) * 0.5 * size;
    let field = pixels(dir, doc, bend["normal"].as_str()).map(|m| crate::rigs::url(&m));
    let normal = pixels(dir, doc, r["normal"].as_str());
    json!({ "system": id, "lit": lit, "normal": r["normal"], "refract": bend, "pictures": pictures,
        "sides": crate::rigs::url(&sides(normal.as_ref())), "field": field, "size": size, "shift": shift,
        "levels": levels(lit, bend.is_object()), "phone": { "tier": Tier::T1, "lights": super::FEW,
            "refract": Quality::Q2.bend().filter(|_| bend.is_object()), "off_below": Quality::Q2 } })
}
