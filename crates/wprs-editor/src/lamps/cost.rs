use super::placed::placed;
use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{MAX_LIGHTS, Quality, Tier};

const LEVELS: [Quality; 4] = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3];

type Systems = Vec<(String, f64)>;

fn swarms(doc: &Value) -> (Systems, Systems) {
    let systems = doc["scene"]["particles"].as_object().cloned().unwrap_or_default();
    let (mut lit, mut bent) = (Vec::new(), Vec::new());
    for (_, l) in crate::layers::walk(doc).into_iter().filter(|(_, l)| l["type"] == "particles") {
        let id = l["system"].as_str().unwrap_or_default();
        let r = if l["render"].is_object() {
            &l["render"]
        } else {
            &systems.get(id).unwrap_or(&Value::Null)["render"]
        };
        let n = systems.get(id).and_then(|s| s["capacity"].as_f64()).unwrap_or(0.0);
        if r["lit"] == true || l.get("lighting").is_some() {
            lit.push((id.to_owned(), n));
        }
        if r["refract"].is_object() {
            bent.push((id.to_owned(), n));
        }
    }
    (lit, bent)
}

pub fn cost(dir: &Path, doc: &Value, t: f64, quality: Quality) -> Value {
    let all = placed(dir, doc, t);
    let lights = all.len().min(MAX_LIGHTS);
    let lit: Vec<_> = crate::canvas::gizmos(dir, doc, t)
        .into_iter()
        .filter(|g| doc.pointer(&g.pointer).is_some_and(|l| l.get("lighting").is_some()))
        .collect();
    let pixels: f64 = lit.iter().map(|g| (g.size[0] * g.scale[0] * g.size[1] * g.scale[1]).abs()).sum();
    let layers = crate::layers::walk(doc);
    let animated = layers.iter().any(|(_, l)| l["type"] == "light" && super::moving(l));
    let airy: Vec<(f64, f64)> = all
        .iter()
        .filter_map(|p| {
            let (_, l) = crate::actions::layer(doc, &p.id).ok()?;
            let samples = l["volumetric"]["samples"].as_f64().or(l.get("volumetric").map(|_| 16.0))?;
            Some((f64::from(super::report::reach(p)[2..].iter().product::<f32>()), samples))
        })
        .collect();
    let (lit_swarms, bent) = swarms(doc);
    let canvas = doc["scene"]["canvas"]["size"]
        .as_array()
        .map_or(1920.0 * 1080.0, |s| s.iter().filter_map(Value::as_f64).product::<f64>());
    let level = |q: Quality, tier: Tier| {
        let baked = q == Quality::Q0;
        let per_frame = if baked || !animated { 0.0 } else { pixels * lights as f64 };
        let air = q.air(tier).map_or(0.0, |(s, k)| {
            airy.iter().map(|(px, n)| px / f64::from(s * s) * n * f64::from(k)).sum::<f64>()
        });
        let cap = if tier <= Tier::T1 { super::FEW } else { MAX_LIGHTS };
        let shaded: f64 = lit_swarms.iter().map(|(_, n)| n * lights.min(super::FEW) as f64).sum();
        let bend = q.bend().map_or(0.0, |k| canvas * (1.0 + f64::from(k * k)) * bent.len().min(1) as f64);
        json!({ "quality": q, "baked": baked, "lights": lights.min(cap), "lit_layers": lit.len(),
            "shaded": round(pixels as f32 * lights as f32), "per_frame": round(per_frame as f32),
            "air": q.air(tier).map(|(s, k)| json!({ "scale": 1.0 / s, "samples": k })), "air_taps": round(air as f32),
            "lit_particles": round(shaded as f32), "refract": q.bend().filter(|_| !bent.is_empty()),
            "refract_pixels": round(bend as f32) })
    };
    let rows = LEVELS.map(|q| level(q, Tier::T3));
    let phone = LEVELS.map(|q| level(q, Tier::T1));
    let named = |list: &[(String, f64)]| list.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    json!({ "quality": quality, "levels": rows, "phone": phone, "animated": animated,
        "lit_layers": lit.iter().map(|g| &g.id).collect::<Vec<_>>(), "volumetric": airy.len(),
        "lit_systems": named(&lit_swarms), "refracting": named(&bent),
        "tiers": super::report::tiers(layers.iter().filter(|(_, l)| l["type"] == "light").count()) })
}
