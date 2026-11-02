use super::placed::placed;
use crate::ops::{Op, set};
use crate::rigs::round;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Beam, Quality, Tier, Volumetric};

const LEVELS: [Quality; 4] = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3];

pub(crate) fn volumetric(
    doc: &Value,
    (path, beam): (&str, Beam),
    (density, samples, shafts): (Option<f64>, Option<u32>, Option<f64>),
    off: bool,
) -> Result<Vec<Op>> {
    let at = format!("{path}/volumetric");
    if off {
        return Ok(doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect());
    }
    ensure!(
        matches!(beam, Beam::Point | Beam::Spot),
        said("editor.light_air", "only point and spot lights light the air", &[])
    );
    let base = Volumetric::default();
    let old = |k: &str| doc.pointer(&format!("{at}/{k}")).and_then(Value::as_f64);
    let density = density.or(old("density")).unwrap_or((f64::from(base.density) * 1e4).round() / 1e4);
    let samples = samples.or(old("samples").map(|n| n as u32)).unwrap_or(base.samples);
    let shafts = shafts.or(old("shafts")).unwrap_or(0.0);
    let unit = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
    ensure!(
        unit(density) && unit(shafts) && (4..=64).contains(&samples),
        said("editor.light_air_range", "density and shafts go from 0 to 1 and samples from 4 to 64", &[])
    );
    let mut air = json!({ "density": density, "samples": samples });
    if shafts > 0.0 {
        air["shafts"] = json!(shafts);
    }
    Ok(set(doc, &at, air))
}

pub fn levels(samples: u32, tier: Tier) -> Value {
    let rows = LEVELS.map(|q| match q.air(tier) {
        None => json!({ "quality": q, "on": false }),
        Some((step, share)) => json!({ "quality": q, "on": true, "scale": 1.0 / step,
            "samples": ((samples as f32 * share).round() as u32).max(4) }),
    });
    json!(rows)
}

pub fn air(dir: &Path, doc: &Value, id: &str, t: f64) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Some(l) = placed(dir, doc, t).into_iter().find(|l| l.id == id) else { return Value::Null };
    let v: Option<Volumetric> = serde_json::from_value(layer["volumetric"].clone()).ok();
    let Some(v) = v else {
        return json!({ "on": false, "fits": matches!(l.beam, Beam::Point | Beam::Spot) });
    };
    let reach = v.density;
    let glow = |d: f64| {
        let d = d as f32;
        let n = v.samples.max(1);
        let sum: f32 = (0..n).map(|i| l.power(d * (1.0 - reach * i as f32 / n as f32))).sum();
        let edge = (1.0 - (d / l.radius.max(1e-3)).powi(2)).clamp(0.0, 1.0);
        f64::from(sum / n as f32 * reach * edge)
    };
    let curve = crate::curves::plot([0.0, f64::from(l.radius)], 48, glow);
    let pixels = super::report::reach(&l)[2..].iter().product::<f32>();
    let taps = [Quality::Q1, Quality::Q2, Quality::Q3]
        .map(|q| q.air(Tier::T3).map_or(0.0, |(s, k)| round(pixels / (s * s) * v.samples as f32 * k)));
    json!({ "on": true, "fits": true, "density": v.density, "samples": v.samples, "shafts": v.shafts, "curve": curve,
        "marks": [round(l.radius * (1.0 - reach))], "box": super::report::reach(&l).map(round),
        "levels": levels(v.samples, Tier::T3), "phone": levels(v.samples, Tier::T1), "taps": taps })
}
