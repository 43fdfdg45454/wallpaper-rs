use crate::action::{Action, ModelEdit, ModelSet};
use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

fn finite(name: &str, v: &[f64], lo: f64, hi: f64) -> Result<()> {
    ensure!(
        v.iter().all(|x| x.is_finite() && (lo..=hi).contains(x)),
        said("editor.model_range", "%1 of a model goes from %2 to %3", &[&name, &lo, &hi])
    );
    Ok(())
}

pub(crate) fn model_layer<'a>(doc: &'a Value, id: &str) -> Result<(String, &'a Value)> {
    let (path, layer) = crate::actions::layer(doc, id)?;
    ensure!(layer["type"] == "model", said("editor.not_model_layer", "%1 is not a model layer", &[&id]));
    Ok((path, layer))
}

fn round(v: &[f64]) -> Value {
    json!(v.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>())
}

fn change(dir: &Path, doc: &Value, s: &ModelSet) -> Result<Vec<Op>> {
    let (path, _) = model_layer(doc, &s.layer)?;
    let mut ops = Vec::new();
    for (field, given) in [("position", s.position), ("rotation", s.rotation), ("scale", s.scale)] {
        let held = doc.pointer(&format!("{path}/{field}")).is_some_and(Value::is_object);
        ensure!(
            given.is_none() || !held,
            said(
                "editor.model_held",
                "the %1 of the model is keyed or bound: add a keyframe or change the binding",
                &[&field]
            )
        );
    }
    let mut put = |field: &str, v: Value| ops.extend(set(doc, &format!("{path}/{field}"), v));
    if let Some(p) = s.position {
        finite("position", &p, -1e6, 1e6)?;
        put("position", round(&p));
    }
    if let Some(r) = s.rotation {
        finite("rotation", &r, -3600.0, 3600.0)?;
        put("rotation", round(&r));
    }
    if let Some(k) = s.scale {
        finite("scale", &k, -1e4, 1e4)?;
        ensure!(
            k.iter().all(|v| *v != 0.0),
            said("editor.model_flat", "a model cannot be scaled to nothing", &[])
        );
        put("scale", round(&k));
    }
    if let Some(size) = s.size {
        finite("size", &size, 1.0, 1e5)?;
        put("size", round(&size));
    }
    if let Some(a) = &s.asset {
        super::store::asset(dir, doc, a)?;
        put("model", json!(a));
    }
    match s.env.as_deref() {
        None => {}
        Some("") => ops
            .extend(doc.pointer(&format!("{path}/env")).map(|_| Op::Remove { path: format!("{path}/env") })),
        Some(e) => {
            crate::lamps::picture(doc, e)?;
            ops.extend(set(doc, &format!("{path}/env"), json!(e)));
        }
    }
    Ok(ops)
}

pub(crate) fn model(dir: &Path, doc: &Value, e: &ModelEdit) -> Result<Vec<Op>> {
    Ok(match e {
        ModelEdit::Add { asset, id, into, at } => {
            super::store::asset(dir, doc, asset)?;
            let id = id.clone().unwrap_or_else(|| crate::layers::unique(doc, asset));
            let add = Action::AddLayer {
                kind: "model".into(),
                id: Some(id),
                name: None,
                source: Some(asset.clone()),
                into: into.clone(),
                at: None,
                file: None,
            };
            let mut ops = crate::actions::plan(doc, &add)?;
            if let (Some(at), Some(Op::Add { value, .. })) = (at, ops.last_mut()) {
                finite("position", at, -1e6, 1e6)?;
                value["position"] = round(at);
            }
            ops
        }
        ModelEdit::Set(s) => change(dir, doc, s)?,
        ModelEdit::Remove { layer } => {
            model_layer(doc, layer)?;
            crate::actions::plan(doc, &Action::RemoveLayer { id: layer.clone() })?
        }
    })
}

pub(crate) fn group(
    doc: &Value,
    id: &Option<String>,
    into: &Option<String>,
    layers: &[String],
) -> Result<Vec<Op>> {
    let id = id.clone().unwrap_or_else(|| crate::layers::unique(doc, "world"));
    Ok(if layers.is_empty() {
        let add = Action::AddLayer {
            kind: "group3d".into(),
            id: Some(id),
            name: None,
            source: None,
            into: into.clone(),
            at: None,
            file: None,
        };
        crate::actions::plan(doc, &add)?
    } else {
        let made = crate::actions::plan(doc, &Action::Group { ids: layers.to_vec(), id: Some(id.clone()) })?;
        let mut after = doc.clone();
        crate::ops::apply(&mut after, &made)?;
        let (path, _) = crate::actions::layer(&after, &id)?;
        let mut ops = made;
        ops.extend(set(&after, &format!("{path}/space"), json!("3d")));
        ops
    })
}

pub(crate) fn space(doc: &Value, layer: &str, deep: bool) -> Result<Vec<Op>> {
    let (path, found) = crate::actions::layer(doc, layer)?;
    ensure!(found["type"] == "group", said("editor.not_group", "%1 is not a group", &[&layer]));
    Ok(match deep {
        true => set(doc, &format!("{path}/space"), json!("3d")),
        false => {
            found.get("space").map(|_| Op::Remove { path: format!("{path}/space") }).into_iter().collect()
        }
    })
}

pub(crate) fn depth_test(doc: &Value, layer: &str, on: bool) -> Result<Vec<Op>> {
    let (path, found) = crate::actions::layer(doc, layer)?;
    ensure!(
        !matches!(found["type"].as_str(), Some("model" | "light" | "sound")),
        said("editor.depth_test_flat", "only flat layers test depth", &[])
    );
    let inside = crate::layers::walk(doc).into_iter().any(|(p, l)| {
        l["type"] == "group" && l["space"] == "3d" && path.starts_with(&format!("{p}/layers/"))
    });
    ensure!(inside || !on, said("editor.depth_test_group", "only layers inside a 3D group test depth", &[]));
    Ok(match on {
        true => set(doc, &format!("{path}/depth_test"), json!(true)),
        false => found
            .get("depth_test")
            .map(|_| Op::Remove { path: format!("{path}/depth_test") })
            .into_iter()
            .collect(),
    })
}
