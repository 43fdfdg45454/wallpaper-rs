use super::view::{View, camera};
use crate::action::{PathEdit, PathKey};
use crate::clips::num;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{MAX_KEYS, MAX_PATHS};

mod flown;

pub use flown::paths;

const ROOT: &str = "/scene/camera3d";

fn view_key(doc: &Value, t: f64) -> Value {
    let raw = &doc["scene"];
    let base = camera(raw);
    let eye = View::read(raw).filter(|v| !v.through).map_or(base, |v| v.eye(&base));
    let r = |v: [f32; 3]| json!(v.map(num));
    json!({ "t": t, "eye": r(eye.at), "center": r(eye.center), "up": r(eye.up), "fov": num(eye.fov) })
}

fn found(paths: &[Value], id: &str) -> Result<usize> {
    paths
        .iter()
        .position(|p| p["id"] == id)
        .ok_or_else(|| anyhow!(said("editor.no_path", "there is no camera path \"%1\"", &[&id])))
}

fn time(k: &Value) -> f64 {
    k["t"].as_f64().unwrap_or(0.0)
}

fn keyed(doc: &Value, keys: &mut Vec<Value>, k: &PathKey) -> Result<()> {
    let fine = |t: f64| t.is_finite() && (0.0..=3600.0).contains(&t);
    ensure!(
        fine(k.time) && k.from.is_none_or(fine),
        said("editor.time_must_more", "the time must be 0 or more seconds", &[])
    );
    let near = |keys: &[Value], t: f64| keys.iter().position(|x| (time(x) - t).abs() < 1e-3);
    if k.remove {
        let i = near(keys, k.time)
            .ok_or_else(|| anyhow!(said("editor.no_path_key", "the path has no key at %1 s", &[&k.time])))?;
        ensure!(keys.len() > 2, said("editor.path_two", "a camera path keeps at least 2 keys", &[]));
        keys.remove(i);
        return Ok(());
    }
    let mut key = match k.from {
        Some(f) => keys.remove(
            near(keys, f)
                .ok_or_else(|| anyhow!(said("editor.no_path_key", "the path has no key at %1 s", &[&f])))?,
        ),
        None => match near(keys, k.time) {
            Some(i) if !k.from_view => keys.remove(i),
            Some(i) => {
                keys.remove(i);
                view_key(doc, k.time)
            }
            None => view_key(doc, k.time),
        },
    };
    key["t"] = json!(num(k.time as f32));
    for (name, v) in [("eye", k.eye), ("center", k.center), ("up", k.up)] {
        if let Some(v) = v {
            ensure!(
                v.iter().all(|x| x.is_finite() && x.abs() <= 1e6),
                said("editor.path_point", "a key point is within a million units", &[])
            );
            key[name] = json!(v.map(|x| num(x as f32)));
        }
    }
    if let Some(f) = k.fov {
        ensure!(
            (1.0..=179.0).contains(&f),
            said("editor.camera_fov", "the field of view goes from 1 to 179 degrees", &[])
        );
        key["fov"] = num(f as f32);
    }
    ensure!(
        keys.len() < MAX_KEYS,
        said("editor.path_keys", "a camera path has at most %1 keys", &[&MAX_KEYS])
    );
    keys.insert(keys.partition_point(|x| time(x) <= k.time), key);
    Ok(())
}

pub(crate) fn edit(_dir: &Path, doc: &Value, e: &PathEdit) -> Result<Vec<Op>> {
    let mut cam = doc.pointer(ROOT).cloned().unwrap_or_else(|| json!({}));
    let mut paths = cam["paths"].as_array().cloned().unwrap_or_default();
    match e {
        PathEdit::Add { id } => {
            crate::clips::valid(id)?;
            ensure!(
                found(&paths, id).is_err(),
                said("editor.path_taken", "there is already a camera path \"%1\"", &[id])
            );
            ensure!(
                paths.len() < MAX_PATHS,
                said("editor.paths_most", "a scene has at most %1 camera paths", &[&MAX_PATHS])
            );
            paths.push(json!({ "id": id, "keys": [view_key(doc, 0.0), view_key(doc, 2.0)] }));
        }
        PathEdit::Key(k) => {
            let i = found(&paths, &k.path)?;
            let mut keys = paths[i]["keys"].as_array().cloned().unwrap_or_default();
            keyed(doc, &mut keys, k)?;
            paths[i]["keys"] = json!(keys);
        }
        PathEdit::Remove { path } => drop(paths.remove(found(&paths, path)?)),
        PathEdit::Set { path, closed, ease, to } => {
            let i = found(&paths, path)?;
            let p = &mut paths[i];
            if let Some(c) = closed {
                match c {
                    true => p["loop"] = json!(true),
                    false => drop(p.as_object_mut().and_then(|m| m.remove("loop"))),
                }
            }
            if let Some(ease) = ease {
                serde_json::from_value::<wprs_scene::Ease>(json!(ease))
                    .map_err(|_| anyhow!(said("editor.path_ease", "%1 is not an easing", &[ease])))?;
                p["ease"] = json!(ease);
            }
            if let Some(to) = to.as_deref().filter(|t| *t != path) {
                crate::clips::valid(to)?;
                ensure!(
                    found(&paths, to).is_err(),
                    said("editor.path_taken", "there is already a camera path \"%1\"", &[&to])
                );
                paths[i]["id"] = json!(to);
            }
        }
        PathEdit::Order { order } => {
            ensure!(
                ["sequential", "random"].contains(&order.as_str()),
                said("editor.path_order", "paths play sequential or random", &[])
            );
            cam["order"] = json!(order);
        }
        PathEdit::Fade { to, duration, clear } => match clear {
            true => drop(cam.as_object_mut().and_then(|m| m.remove("fade"))),
            false => {
                let mut fade = cam["fade"].as_object().cloned().unwrap_or_default();
                if let Some(t) = to {
                    ensure!(
                        ["black", "cross"].contains(&t.as_str()),
                        said("editor.path_fade", "paths fade to black or cross", &[])
                    );
                    fade.insert("to".into(), json!(t));
                }
                let d =
                    duration.unwrap_or_else(|| fade.get("duration").and_then(Value::as_f64).unwrap_or(1.0));
                ensure!(
                    d.is_finite() && (0.0..=10.0).contains(&d),
                    said("editor.path_fade_time", "a fade lasts 0 to 10 s", &[])
                );
                fade.insert("duration".into(), num(d as f32));
                cam["fade"] = json!(fade);
            }
        },
    }
    match paths.is_empty() {
        true => drop(cam.as_object_mut().and_then(|m| m.remove("paths"))),
        false => cam["paths"] = json!(paths),
    }
    Ok(set(doc, ROOT, cam))
}
