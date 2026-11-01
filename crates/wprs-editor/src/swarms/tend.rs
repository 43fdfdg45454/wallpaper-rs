use super::{parsed, put, render, system};
use crate::action::{Response, Strand, TendEdit};
use crate::ops::{Op, set};
use anyhow::{Result, bail, ensure};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wprs_core::message::{Message, said};
use wprs_scene::{Collider, Operator};

type Missing = fn(usize) -> Message;

pub(super) fn listed<T: DeserializeOwned>(
    doc: &Value,
    at: &str,
    (index, value, remove): (Option<usize>, Option<&Value>, bool),
    (what, missing): (&str, Missing),
) -> Result<Vec<Op>> {
    let list = doc.pointer(at).and_then(Value::as_array).cloned().unwrap_or_default();
    match (index, value, remove) {
        (Some(i), _, true) => {
            ensure!(i < list.len(), missing(i));
            Ok(vec![Op::Remove { path: if list.len() == 1 { at.into() } else { format!("{at}/{i}") } }])
        }
        (i, Some(v), false) => {
            parsed::<T>(v, what)?;
            let i = i.unwrap_or(list.len());
            ensure!(i <= list.len(), missing(i));
            let mut next = list;
            if i == next.len() {
                next.push(v.clone())
            } else {
                next[i] = v.clone()
            }
            Ok(set(doc, at, json!(next)))
        }
        _ => bail!(said("editor.list_what", "give what to add or set, or the index of one to remove", &[])),
    }
}

fn merged(base: Value, change: Option<&Value>) -> Value {
    let mut out = base;
    for (k, v) in change.and_then(Value::as_object).into_iter().flatten() {
        out[k] = v.clone();
    }
    out
}

fn respond(
    doc: &Value,
    sys: &str,
    index: usize,
    (r, bounce, friction): (Response, Option<f64>, Option<f64>),
) -> Result<Vec<Op>> {
    let at = format!("{sys}/collide/{index}");
    let Some(old) = doc.pointer(&at) else {
        bail!(said("editor.no_collider", "the system has no collider %1", &[&index]))
    };
    let mut next = old.clone();
    next.as_object_mut().into_iter().for_each(|o| ["kill", "stick"].iter().for_each(|k| drop(o.remove(*k))));
    let kept = old["bounce"].as_f64().filter(|b| *b > 0.0).unwrap_or(0.5);
    let (b, f) = match r {
        Response::Bounce => (bounce.unwrap_or(kept), friction),
        Response::Slide => (0.0, friction.or(Some(0.0))),
        Response::Stop => (0.0, Some(1.0)),
        Response::Stick | Response::Kill => (0.0, None),
    };
    next["bounce"] = json!(b);
    f.iter().for_each(|f| next["friction"] = json!(f));
    match r {
        Response::Stick => next["stick"] = json!(true),
        Response::Kill => next["kill"] = json!(true),
        _ => {}
    }
    parsed::<Collider>(&next, "collider")?;
    Ok(set(doc, &at, next))
}

fn flock(doc: &Value, sys: &str, change: Option<&Value>, off: bool) -> Result<Vec<Op>> {
    let at = format!("{sys}/forces");
    let mut list = doc.pointer(&at).and_then(Value::as_array).cloned().unwrap_or_default();
    let found = list.iter().position(|f| f.get("boids").is_some());
    if off {
        let path = |i| if list.len() == 1 { at.clone() } else { format!("{at}/{i}") };
        return Ok(found.map(|i| vec![Op::Remove { path: path(i) }]).unwrap_or_default());
    }
    let fresh = json!({ "boids": { "radius": 40, "separation": 1, "alignment": 1, "cohesion": 0.5, "max_speed": 200 } });
    let mut entry = found.map_or(fresh, |i| list[i].clone());
    entry["boids"] = merged(entry["boids"].take(), change);
    parsed::<Operator>(&entry, "boids")?;
    match found {
        Some(i) => list[i] = entry,
        None => list.push(entry),
    }
    Ok(set(doc, &at, json!(list)))
}

fn strand(
    doc: &Value,
    sys: &str,
    (kind, layer): (Strand, Option<&str>),
    settings: Option<&Value>,
    off: bool,
) -> Result<Vec<Op>> {
    let (key, fresh) = match kind {
        Strand::Rope => ("rope", json!({})),
        Strand::Trail => ("trail", json!({ "length": 32, "width": 0.5 })),
    };
    render(doc, sys, layer, |d, at| {
        let path = format!("{at}/{key}");
        let next = (!off).then(|| merged(d.pointer(&path).cloned().unwrap_or(fresh.clone()), settings));
        put(d, &path, next)
    })
}

pub(crate) fn edit(doc: &Value, e: &TendEdit) -> Result<Vec<Op>> {
    let no_collider: Missing = |i| said("editor.no_collider", "the system has no collider %1", &[&i]);
    let no_force: Missing = |i| said("editor.no_force", "the system has no force %1", &[&i]);
    match e {
        TendEdit::Collider { system: s, index, collider, remove } => {
            let at = format!("{}/collide", system(doc, s)?);
            listed::<Collider>(doc, &at, (*index, collider.as_ref(), *remove), ("collider", no_collider))
        }
        TendEdit::Response { system: s, index, response, bounce, friction } => {
            respond(doc, &system(doc, s)?, *index, (*response, *bounce, *friction))
        }
        TendEdit::Force { system: s, index, force, remove } => {
            let at = format!("{}/forces", system(doc, s)?);
            listed::<Operator>(doc, &at, (*index, force.as_ref(), *remove), ("force", no_force))
        }
        TendEdit::Boids { system: s, boids, off } => flock(doc, &system(doc, s)?, boids.as_ref(), *off),
        TendEdit::Strand { system: s, strand: kind, layer, settings, off } => {
            strand(doc, &system(doc, s)?, (*kind, layer.as_deref()), settings.as_ref(), *off)
        }
        TendEdit::Texture { system: s, layer, texture } => {
            render(doc, &system(doc, s)?, layer.as_deref(), |d, at| {
                put(d, &format!("{at}/texture"), texture.as_ref().map(|t| json!(t)))
            })
        }
    }
}
