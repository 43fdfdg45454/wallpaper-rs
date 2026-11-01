mod handles;
mod looks;
mod samples;
mod shapes;
mod spots;

use crate::action::ParticleEdit;
use crate::ops::{Op, apply, escape, set};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

pub use crate::curves::{Curve, plot};
pub use handles::handles;
pub use looks::{controls, sources};

fn system(doc: &Value, id: &str) -> Result<String> {
    let path = format!("/scene/particles/{}", escape(id));
    ensure!(
        doc.pointer(&path).is_some(),
        said("editor.no_system", "there is no particle system \"%1\"", &[&id])
    );
    Ok(path)
}

fn render(
    doc: &Value,
    sys: &str,
    layer: Option<&str>,
    change: impl Fn(&Value, &str) -> Vec<Op>,
) -> Result<Vec<Op>> {
    let Some(id) = layer else { return Ok(change(doc, &format!("{sys}/render"))) };
    let (path, found) = crate::actions::layer(doc, id)?;
    let fits = found["type"] == "particles" && found["system"] == sys.rsplit('/').next().unwrap_or_default();
    ensure!(fits, said("editor.not_particles", "the layer \"%1\" draws no particles of this system", &[&id]));
    let own = format!("{path}/render");
    let copy = doc.pointer(&format!("{sys}/render")).cloned().unwrap_or(json!({}));
    let mut ops = if found.get("render").is_some() { Vec::new() } else { set(doc, &own, copy) };
    let mut probe = doc.clone();
    apply(&mut probe, &ops)?;
    ops.extend(change(&probe, &own));
    Ok(ops)
}

fn put(doc: &Value, path: &str, value: Option<Value>) -> Vec<Op> {
    match value {
        Some(v) => set(doc, path, v),
        None if doc.pointer(path).is_some() => vec![Op::Remove { path: path.into() }],
        None => Vec::new(),
    }
}

fn parsed<T: serde::de::DeserializeOwned>(v: &Value, what: &str) -> Result<()> {
    serde_json::from_value::<T>(v.clone())
        .map(drop)
        .map_err(|e| anyhow!(said("editor.particle_value", "%1 does not fit: %2", &[&what, &e])))
}

fn issues(dir: &Path, doc: &Value) -> Result<Vec<(String, String)>> {
    let mut scene: wprs_scene::Scene = serde_json::from_value(doc["scene"].clone())?;
    let unread = crate::cache::read_files(&mut scene, dir);
    let props = crate::meta::read(doc, Some(dir)).map(|m| m.properties).unwrap_or_default();
    let all = unread.into_iter().chain(scene.validate(&props));
    Ok(all.map(|i| (i.to_string(), i.message().to_string())).collect())
}

fn checked(dir: &Path, doc: &Value, ops: Vec<Op>) -> Result<Vec<Op>> {
    let mut probe = doc.clone();
    apply(&mut probe, &ops)?;
    let before = issues(dir, doc).unwrap_or_default();
    let fresh = issues(dir, &probe)?.into_iter().find(|i| !before.contains(i));
    if let Some((_, message)) = fresh {
        bail!(message);
    }
    Ok(ops)
}

pub(crate) fn edit(dir: &Path, doc: &Value, e: &ParticleEdit) -> Result<Vec<Op>> {
    let ops = match e {
        ParticleEdit::Shape { system: s, emitter, shape } => {
            spots::shape(doc, &system(doc, s)?, *emitter, shape)?
        }
        ParticleEdit::Point { system: s, point, at, follow, angle, remove } => {
            spots::point(doc, &system(doc, s)?, *point, (*at, *follow, *angle), *remove)?
        }
        ParticleEdit::Remap { system: s, index, remap, remove } => {
            looks::remap(doc, &system(doc, s)?, *index, remap.as_ref(), *remove)?
        }
        ParticleEdit::Cutout { system: s, layer, range } => {
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                put(d, &format!("{at}/cutout"), range.map(|r| json!(r)))
            })?
        }
        ParticleEdit::Orient { system: s, layer, orient, angle } => {
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                let fixed = *orient == Some(wprs_scene::Orient::Fixed);
                let mut ops = put(d, &format!("{at}/orient"), orient.map(|o| json!(o)));
                ops.extend(put(d, &format!("{at}/angle"), angle.filter(|_| fixed).map(|a| json!(a))));
                ops
            })?
        }
        ParticleEdit::Perspective { system: s, focal, depth, depth_speed, off } => {
            looks::perspective(doc, &system(doc, s)?, *focal, (*depth, *depth_speed), *off)
        }
        ParticleEdit::Layer { system: s, emitter, id, threshold, color, inherit, jitter, refresh } => {
            let mut source = json!({ "id": id, "color": color, "inherit": inherit });
            threshold.iter().for_each(|t| source["threshold"] = json!(t));
            jitter.iter().for_each(|j| source["jitter"] = json!(j));
            refresh.iter().for_each(|r| source["refresh"] = json!(r));
            spots::shape(doc, &system(doc, s)?, *emitter, &json!({ "layer": source }))?
        }
        ParticleEdit::Inherit { system: s, child, inherit, follow } => {
            let at = format!("{}/children/{child}", system(doc, s)?);
            ensure!(
                doc.pointer(&at).is_some(),
                said("editor.no_child", "the system has no child %1", &[child])
            );
            let mut ops = put(doc, &format!("{at}/inherit"), (!inherit.is_empty()).then(|| json!(inherit)));
            ops.extend(put(doc, &format!("{at}/follow_inherit"), follow.filter(|f| *f).map(|_| json!(true))));
            ops
        }
        ParticleEdit::Shader { system: s, layer, shader } => {
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                put(d, &format!("{at}/shader"), shader.as_ref().map(|id| json!(id)))
            })?
        }
    };
    checked(dir, doc, ops)
}
