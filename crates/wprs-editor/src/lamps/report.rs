use super::placed::{Placed, placed};
use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{Beam, MAX_LIGHTS, Tier};

pub const Z_KNOB: f32 = 0.5;
pub const AIM: f32 = 160.0;
pub const FEW: usize = 8;

fn r2(p: [f32; 2]) -> [f64; 2] {
    p.map(round)
}

fn toward(p: [f32; 2], degrees: f32, d: f32) -> [f32; 2] {
    let (s, c) = degrees.to_radians().sin_cos();
    [p[0] + c * d, p[1] + s * d]
}

pub fn reach(l: &Placed) -> [f32; 4] {
    let at = l.at();
    let half = if l.beam == Beam::Tube { l.length / 2.0 } else { 0.0 };
    let [a, b] = [toward(at, l.angle(), half), toward(at, l.angle() + 180.0, half)];
    let (x0, y0) = (a[0].min(b[0]) - l.radius, a[1].min(b[1]) - l.radius);
    [x0, y0, (a[0] - b[0]).abs() + 2.0 * l.radius, (a[1] - b[1]).abs() + 2.0 * l.radius]
}

pub fn handles(l: &Placed) -> Value {
    let (at, angle) = (l.at(), l.angle());
    let mut out = json!({
        "id": l.id, "type": l.beam, "at": r2(at), "z": round(l.z), "radius": round(l.radius),
        "intensity": round(l.intensity), "color": l.color.map(round), "angle": round(angle),
        "base": l.base(), "tilt": round(l.tilt), "heading": l.heading.map(round), "parent": l.parent.0.map(round),
        "z_knob": r2([at[0], at[1] - l.z * Z_KNOB]), "box": reach(l).map(round),
    });
    match l.beam {
        Beam::Spot => {
            out["aim"] = json!(r2(toward(at, angle, AIM.min(l.radius))));
            let edge = |k: f32, s: f32| r2(toward(at, angle + s * k / 2.0, AIM.min(l.radius)));
            out["cone"] = json!(l.cone.map(round));
            out["outer"] = json!([edge(l.cone[1], -1.0), edge(l.cone[1], 1.0)]);
            out["inner"] = json!([edge(l.cone[0], -1.0), edge(l.cone[0], 1.0)]);
        }
        Beam::Tube => {
            let half = l.length / 2.0;
            out["length"] = json!(round(l.length));
            out["ends"] = json!([r2(toward(at, angle + 180.0, half)), r2(toward(at, angle, half))]);
            out["radius_knob"] = json!(r2(toward(at, angle + 90.0, l.radius)));
        }
        Beam::Directional => out["aim"] = json!(r2(toward(at, angle, AIM))),
        Beam::Point => {}
    }
    if l.beam != Beam::Tube {
        out["radius_knob"] = json!(r2(toward(at, 0.0, l.radius)));
    }
    out
}

pub fn tiers(count: usize) -> Value {
    let rows = [Tier::T0, Tier::T1, Tier::T2, Tier::T3].map(|tier| {
        let cap = if tier <= Tier::T1 { FEW } else { MAX_LIGHTS };
        json!({ "tier": tier, "cap": cap, "kept": count.min(cap), "left_out": count.saturating_sub(cap) })
    });
    json!(rows)
}

pub fn lights(dir: &Path, doc: &Value, t: f64) -> Value {
    let found = placed(dir, doc, t);
    let all: Vec<&Value> =
        crate::layers::walk(doc).into_iter().map(|(_, l)| l).filter(|l| l["type"] == "light").collect();
    let ext = &doc["scene"]["ext"]["editor"]["lights"];
    let muted: Vec<&str> = ext["muted"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let solo = ext["solo"].as_str();
    let rows = found.iter().map(|l| {
        let mut h = handles(l);
        h["pointer"] = json!(crate::layers::find(doc, &l.id).unwrap_or_default());
        h["muted"] = json!(muted.contains(&l.id.as_str()));
        h["solo"] = json!(solo == Some(l.id.as_str()));
        let own = crate::actions::layer(doc, &l.id).map(|(_, v)| v).unwrap_or(&Value::Null);
        h["hidden"] = json!(own["visible"] == false);
        (h["volumetric"], h["texture"]) = (own["volumetric"].clone(), own["texture"].clone());
        h
    });
    json!({ "lights": rows.collect::<Vec<_>>(), "count": all.len(), "max": MAX_LIGHTS, "tiers": tiers(all.len()),
        "solo": solo, "muted": muted, "time": t })
}

pub fn falloff(dir: &Path, doc: &Value, id: &str, t: f64) -> Value {
    let Some(l) = placed(dir, doc, t).into_iter().find(|l| l.id == id) else { return Value::Null };
    let curve = crate::curves::plot([0.0, f64::from(l.radius)], 48, |d| f64::from(l.power(d as f32)));
    json!({ "curve": curve, "marks": [round(l.radius / 2.0)], "radius": round(l.radius), "intensity": round(l.intensity) })
}
