use super::placed::placed;
use crate::action::{EDITOR, LightEdit, LightSet};
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::Beam;

pub fn template(doc: &Value, id: &str, source: Option<&str>) -> Result<Value> {
    let beam: Beam = match source {
        None => Beam::Point,
        Some(s) => serde_json::from_value(json!(s)).map_err(|_| {
            anyhow!(said(
                "editor.light_type",
                "%1 is not a light type; use point, spot, tube or directional",
                &[&s]
            ))
        })?,
    };
    let size = &doc["scene"]["canvas"]["size"];
    let middle = [0, 1].map(|i| size[i].as_f64().unwrap_or([1920.0, 1080.0][i]) / 2.0);
    let mut l = json!({ "id": id, "type": "light", "light": beam, "transform": { "position": middle } });
    let extra = match beam {
        Beam::Point => json!({ "radius": 400, "z": 120 }),
        Beam::Spot => json!({ "radius": 900, "z": 150, "intensity": 2, "cone": [25, 45] }),
        Beam::Tube => json!({ "radius": 240, "z": 60, "length": 400 }),
        Beam::Directional => json!({ "intensity": 0.8 }),
    };
    l.as_object_mut().into_iter().for_each(|m| m.extend(extra.as_object().cloned().unwrap_or_default()));
    if matches!(beam, Beam::Spot | Beam::Directional) {
        l["transform"]["tilt"] = json!([if beam == Beam::Spot { 45 } else { 50 }, 0]);
    }
    Ok(l)
}

fn ranged(name: &str, v: f64, lo: f64, hi: f64) -> Result<f64> {
    ensure!(
        v.is_finite() && (lo..=hi).contains(&v),
        said("editor.light_range", "%1 of a light goes from %2 to %3", &[&name, &lo, &hi])
    );
    Ok(v)
}

fn light(doc: &Value, id: &str) -> Result<(String, Beam)> {
    let (path, layer) = crate::actions::layer(doc, id)?;
    ensure!(layer["type"] == "light", said("editor.not_light", "%1 is not a light layer", &[&id]));
    Ok((path, serde_json::from_value(layer["light"].clone()).unwrap_or_default()))
}

fn only(beam: Beam, wanted: Beam, field: &str) -> Result<()> {
    ensure!(
        beam == wanted,
        said("editor.light_field", "only %1 lights take %2", &[&wanted_name(wanted), &field])
    );
    Ok(())
}

fn wanted_name(b: Beam) -> String {
    serde_json::to_value(b).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

pub(crate) fn change(dir: &Path, doc: &Value, s: &LightSet) -> Result<Vec<Op>> {
    let (path, old) = light(doc, &s.layer)?;
    let beam = s.light.unwrap_or(old);
    let mut ops = Vec::new();
    if s.light.is_some() {
        ops.extend(set(doc, &format!("{path}/light"), json!(beam)));
        let wrong = [("cone", Beam::Spot), ("length", Beam::Tube), ("texture", Beam::Spot)];
        let found =
            wrong.into_iter().filter(|(f, k)| *k != beam && doc.pointer(&format!("{path}/{f}")).is_some());
        ops.extend(found.map(|(field, _)| Op::Remove { path: format!("{path}/{field}") }));
    }
    let mut put = |field: &str, v: Value| ops.extend(set(doc, &format!("{path}/{field}"), v));
    if let Some(c) = &s.color {
        serde_json::from_value::<wprs_scene::Color>(c.clone()).map_err(|_| {
            anyhow!(said("editor.light_color", "a light color is #rrggbb or [r, g, b, a]", &[]))
        })?;
        put("color", c.clone());
    }
    let numbers =
        [("intensity", s.intensity, 0.0, 100.0), ("radius", s.radius, 1.0, 1e5), ("z", s.z, -1e5, 1e5)];
    for (name, v, lo, hi) in numbers {
        if let Some(v) = v {
            put(name, json!(ranged(name, v, lo, hi)?));
        }
    }
    if let Some([inner, outer]) = s.cone {
        only(beam, Beam::Spot, "cone")?;
        ranged("cone", inner, 0.0, 180.0)?;
        put("cone", json!([inner, ranged("cone", outer, inner, 180.0)?]));
    }
    if let Some(v) = s.length {
        only(beam, Beam::Tube, "length")?;
        put("length", json!(ranged("length", v, 1.0, 1e5)?));
    }
    if let Some(v) = s.tilt {
        put("transform/tilt", json!([ranged("tilt", v, -89.0, 89.0)?, 0]));
    }
    if s.at.is_some() || s.aim.is_some() {
        let found = placed(dir, doc, 0.0).into_iter().find(|l| l.id == s.layer);
        let p = found.ok_or_else(|| {
            anyhow!(said("editor.light_unplaced", "the light %1 is not shown", &[&s.layer]))
        })?;
        let to_local = |q: [f64; 2]| p.parent.apply([q[0] as f32, q[1] as f32]);
        let at = s.at.map_or_else(|| p.parent.apply(p.at()), to_local);
        if s.at.is_some() {
            put("transform/position", json!(at.map(|v| (f64::from(v) * 100.0).round() / 100.0)));
        }
        if let Some(aim) = s.aim.map(to_local) {
            let angle = (aim[1] - at[1]).atan2(aim[0] - at[0]).to_degrees() - p.base();
            put("transform/rotation", json!((f64::from(angle) * 100.0).round() / 100.0));
        }
    }
    Ok(ops)
}

fn listed(doc: &Value) -> Vec<String> {
    let list = doc.pointer(&format!("{EDITOR}/lights/muted")).and_then(Value::as_array).into_iter().flatten();
    list.filter_map(|v| v.as_str().map(String::from)).collect()
}

pub(crate) fn edit(dir: &Path, doc: &Value, e: &LightEdit) -> Result<Vec<Op>> {
    Ok(match e {
        LightEdit::Add { light, id, into, at } => {
            let name = wanted_name(*light);
            let id = id.clone().unwrap_or_else(|| crate::layers::unique(doc, &name));
            let add = crate::actions::Action::AddLayer {
                kind: "light".into(),
                id: Some(id.clone()),
                name: None,
                source: Some(name),
                into: into.clone(),
                at: None,
            };
            let mut ops = crate::actions::plan(doc, &add)?;
            if let (Some(at), Some(Op::Add { value, .. })) = (at, ops.last_mut()) {
                value["transform"]["position"] = json!(at);
            }
            ops
        }
        LightEdit::Set(s) => change(dir, doc, s)?,
        LightEdit::Remove { layer } => {
            light(doc, layer)?;
            let mut ops =
                crate::actions::plan(doc, &crate::actions::Action::RemoveLayer { id: layer.clone() })?;
            let mut muted = listed(doc);
            if muted.contains(layer) {
                muted.retain(|m| m != layer);
                ops.extend(set(doc, &format!("{EDITOR}/lights/muted"), json!(muted)));
            }
            if doc.pointer(&format!("{EDITOR}/lights/solo")) == Some(&json!(layer)) {
                ops.push(Op::Remove { path: format!("{EDITOR}/lights/solo") });
            }
            ops
        }
        LightEdit::Solo { layer: Some(layer) } => {
            light(doc, layer)?;
            set(doc, &format!("{EDITOR}/lights/solo"), json!(layer))
        }
        LightEdit::Solo { layer: None } => match doc.pointer(&format!("{EDITOR}/lights/solo")) {
            Some(_) => vec![Op::Remove { path: format!("{EDITOR}/lights/solo") }],
            None => Vec::new(),
        },
        LightEdit::Mute { layer, muted } => {
            light(doc, layer)?;
            let mut list = listed(doc);
            list.retain(|m| m != layer);
            if *muted {
                list.push(layer.clone());
            }
            set(doc, &format!("{EDITOR}/lights/muted"), json!(list))
        }
    })
}
