use super::view::{View, camera};
use crate::action::{CameraEdit, EDITOR, TonesEdit, ViewEdit};
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Tonemap, unit3};

fn ranged(name: &str, v: f64, lo: f64, hi: f64) -> Result<f64> {
    ensure!(
        v.is_finite() && (lo..=hi).contains(&v),
        said("editor.camera_range", "%1 of the 3D camera goes from %2 to %3", &[&name, &lo, &hi])
    );
    Ok((v * 1000.0).round() / 1000.0)
}

fn point(name: &str, v: [f64; 3]) -> Result<Value> {
    Ok(json!([
        ranged(name, v[0], -1e6, 1e6)?,
        ranged(name, v[1], -1e6, 1e6)?,
        ranged(name, v[2], -1e6, 1e6)?
    ]))
}

pub(crate) fn camera_edit(doc: &Value, e: &CameraEdit) -> Result<Vec<Op>> {
    let at = "/scene/camera3d";
    if e.clear {
        return Ok(doc.pointer(at).map(|_| Op::Remove { path: at.into() }).into_iter().collect());
    }
    let mut c = doc.pointer(at).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
    let scene = &doc["scene"];
    if e.from_view {
        let eye = View::read(scene).unwrap_or_else(|| View::from(&camera(scene))).eye(&camera(scene));
        let f = |v: [f32; 3]| v.map(|x| (f64::from(x) * 1000.0).round() / 1000.0);
        (c["eye"], c["center"], c["up"]) = (json!(f(eye.at)), json!(f(eye.center)), json!([0, 1, 0]));
    }
    if let Some(v) = e.fov {
        c["fov"] = json!(ranged("fov", v, 1.0, 179.0)?);
    }
    let near = e.near.map(|v| ranged("near", v, 0.001, 1000.0)).transpose()?;
    if let Some(n) = near {
        c["near"] = json!(n);
    }
    if let Some(v) = e.far {
        let low = c["near"].as_f64().unwrap_or(0.1);
        c["far"] = json!(ranged("far", v, low * 1.0001, 1e6)?);
    }
    for (name, v) in [("eye", e.eye), ("center", e.center), ("up", e.up)] {
        if let Some(v) = v {
            c[name] = point(name, v)?;
        }
    }
    ensure!(
        c["up"] != json!([0.0, 0.0, 0.0]),
        said("editor.camera_up", "the up of the 3D camera cannot be zero", &[])
    );
    if let Some(roll) = e.roll {
        let roll = ranged("roll", roll, -180.0, 180.0)?.to_radians() as f32;
        let mut eye = camera(&json!({ "camera3d": c }));
        eye.up = [0.0, 1.0, 0.0];
        let (r, u, _) = eye.basis();
        let up = unit3(std::array::from_fn(|i| u[i] * roll.cos() + r[i] * roll.sin()));
        c["up"] = json!(up.map(|x| (f64::from(x) * 1000.0).round() / 1000.0));
    }
    Ok(set(doc, at, c))
}

pub(crate) fn view_edit(dir: &Path, doc: &Value, e: &ViewEdit) -> Result<Vec<Op>> {
    let at = format!("{EDITOR}/view3d");
    if e.reset {
        return Ok(doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect());
    }
    let scene = &doc["scene"];
    let base = camera(scene);
    let mut v = View::read(scene).unwrap_or_else(|| View::from(&base));
    if let Some([dx, dy]) = e.orbit {
        (v.yaw, v.pitch) = ((v.yaw + dx as f32) % 360.0, (v.pitch + dy as f32).clamp(-89.0, 89.0));
    }
    if let Some([dx, dy]) = e.pan {
        let eye = v.eye(&base);
        let h = scene["canvas"]["size"][1].as_f64().unwrap_or(1080.0) as f32;
        let (r, u, _) = eye.basis();
        let k = eye.unit(h);
        v.target = std::array::from_fn(|i| v.target[i] - r[i] * dx as f32 * k + u[i] * dy as f32 * k);
    }
    if let Some(z) = e.zoom {
        ensure!(z.is_finite() && z > 0.0, said("editor.view_zoom", "a zoom is a factor above 0", &[]));
        v.distance = (v.distance / z as f32).clamp(0.01, 1e5);
    }
    if let Some(t) = e.through {
        v.through = t;
    }
    if let Some(id) = &e.frame {
        let found = super::gizmo::framed(dir, doc, id);
        let (middle, radius) = found.ok_or_else(|| {
            anyhow::anyhow!(said("editor.view_frame", "%1 is not a model that is shown", &[&id]))
        })?;
        let fit = radius / (base.fov.to_radians() * 0.5).tan().max(1e-3);
        (v.target, v.distance) = (middle, (fit * 1.2).max(0.01));
    }
    Ok(set(doc, &at, v.json()))
}

pub(crate) fn tones(doc: &Value, e: &TonesEdit) -> Result<Vec<Op>> {
    let mut ops = Vec::new();
    let was = doc.pointer("/scene/canvas/hdr") == Some(&json!(true));
    let hdr = e.hdr.unwrap_or(was);
    let gone = |path: &str| doc.pointer(path).map(|_| Op::Remove { path: path.into() });
    match (e.hdr, hdr) {
        (Some(_), true) => ops.extend(set(doc, "/scene/canvas/hdr", json!(true))),
        (Some(_), false) => {
            ops.extend(gone("/scene/canvas/hdr").into_iter().chain(gone("/scene/canvas/tonemap")))
        }
        _ => {}
    }
    if let Some(t) = &e.tonemap {
        let picked: Tonemap = serde_json::from_value(json!(t)).map_err(|_| {
            anyhow::anyhow!(said("editor.tonemap", "%1 is not a tone map; use none, aces or agx", &[&t]))
        })?;
        ensure!(hdr || picked == Tonemap::None, said("editor.tonemap_hdr", "tone maps need hdr", &[]));
        if hdr {
            ops.extend(set(doc, "/scene/canvas/tonemap", json!(picked)));
        }
    }
    if let Some(s) = e.split {
        let at = format!("{EDITOR}/tonemap_split");
        ensure!(
            s.is_finite() && (0.0..=1.0).contains(&s),
            said("editor.tonemap_split", "the split goes from 0 to 1", &[])
        );
        ops.extend(match s {
            0.0 => doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect(),
            s => set(doc, &at, json!((s * 1000.0).round() / 1000.0)),
        });
    }
    Ok(ops)
}

pub(crate) fn debug(doc: &Value, show: Option<&str>) -> Result<Vec<Op>> {
    let at = format!("{EDITOR}/debug3d");
    match show {
        Some(s) => {
            ensure!(
                ["mask", "matte", "shadows"].contains(&s),
                said("editor.debug3d", "the 3D view shows mask, matte or shadows", &[])
            );
            Ok(set(doc, &at, json!(s)))
        }
        None => Ok(doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect()),
    }
}
