use crate::action::{ClipKey, KeyEase};
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;

struct Track {
    path: Vec<String>,
    value: Option<Value>,
    rest: Value,
    range: f32,
}

pub(crate) fn num(v: f32) -> Value {
    json!((f64::from(v) * 1e6).round() / 1e6)
}

fn time(k: &Value) -> f64 {
    k[0].as_f64().unwrap_or(0.0)
}

fn ease(e: KeyEase) -> Option<Value> {
    match e {
        KeyEase::Step => Some(json!("step")),
        KeyEase::Linear => None,
        KeyEase::Cubic => Some(json!("in_out_cubic")),
    }
}

fn tracks(k: &ClipKey, known: &wprs_scene::Rigged) -> Result<Vec<Track>> {
    let ClipKey { bone, rotation, position, scale, shape, weight, order, value, .. } = k;
    let one = [bone.is_some(), shape.is_some(), order.is_some()].iter().filter(|x| **x).count() == 1;
    ensure!(one, said("editor.key_what", "a key is for one bone, blend shape or part order", &[]));
    let track = |path: [&str; 3], value: Option<Value>, rest: Value, range: f32| Track {
        path: path.iter().filter(|p| !p.is_empty()).map(|p| p.to_string()).collect(),
        value,
        rest,
        range,
    };
    if let Some(b) = bone {
        known.joint(b).ok_or_else(|| anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[b])))?;
        let all = [
            track(["bones", b, "rotation"], rotation.map(num), json!(0), 1e5),
            track(["bones", b, "position"], position.map(|v| json!(v.map(num))), json!([0, 0]), 1e5),
            track(["bones", b, "scale"], scale.map(|v| json!(v.map(num))), json!([1, 1]), 1e3),
        ];
        let named = rotation.is_some() || position.is_some() || scale.is_some();
        return Ok(all.into_iter().filter(|t| !named || t.value.is_some()).collect());
    }
    if let Some(s) = shape {
        let no = || anyhow!(said("editor.no_shape", "there is no blend shape \"%1\"", &[s]));
        let i = known.shape(s).ok_or_else(no)?;
        let rest = json!(known.defaults.get(i).copied().unwrap_or(0.0));
        return Ok(vec![track(["shapes", s, ""], weight.map(num), rest, 1.0)]);
    }
    let p = order.as_deref().unwrap_or_default();
    known.part(p).ok_or_else(|| anyhow!(said("editor.no_part", "there is no part \"%1\"", &[&p])))?;
    Ok(vec![track(["order", p, ""], value.map(|v| num(v.round())), json!(0), 1000.0)])
}

pub(crate) fn key(dir: &std::path::Path, doc: &Value, k: &ClipKey) -> Result<Vec<Op>> {
    let ClipKey { layer, clip, time: t, ease: e, remove, from, copy, .. } = k;
    let (path, l) = crate::actions::layer(doc, layer)?;
    let (_, _, known) = crate::skin::rigged(dir, doc, l)?;
    let (i, found) = super::found(l, clip)?;
    let at = |t: f64| (t * 1000.0).round() / 1000.0;
    let fine = |t: f64| t.is_finite() && (0.0..=f64::from(super::MAX_LENGTH)).contains(&t);
    ensure!(
        fine(*t) && from.is_none_or(fine),
        said("editor.time_must_more", "the time must be 0 or more seconds", &[])
    );
    let (t, from) = (at(*t), from.map(at));
    let mut c = found.clone();
    let mut done = 0;
    for track in tracks(k, &known)? {
        let pointer = format!("/{}", track.path.iter().map(|p| escape(p)).collect::<Vec<_>>().join("/"));
        let mut keys = c.pointer(&pointer).and_then(Value::as_array).cloned().unwrap_or_default();
        let near = |keys: &[Value], s: f64| keys.iter().position(|k| (time(k) - s).abs() < 1e-3);
        let mut new = None;
        if let Some(s) = from {
            let Some(k) = near(&keys, s) else { continue };
            let old = if *copy { keys[k].clone() } else { keys.remove(k) };
            new = Some((track.value.clone().unwrap_or(old[1].clone()), e.map_or(old.get(2).cloned(), ease)));
        } else if *remove {
            let Some(k) = near(&keys, t) else { continue };
            keys.remove(k);
        } else {
            let held = near(&keys, t).and_then(|k| keys[k].get(2).cloned());
            let v =
                track.value.clone().or_else(|| crate::keys::sample(&keys, t)).unwrap_or(track.rest.clone());
            new = Some((v, e.map_or(held, ease)));
        }
        if let Some((v, eased)) = new {
            let within =
                |x: &Value| x.as_f64().is_some_and(|x| x.is_finite() && x.abs() <= f64::from(track.range));
            let ok = within(&v) || v.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(within));
            ensure!(ok, said("editor.key_value", "the key holds a value out of its range", &[]));
            keys.retain(|k| (time(k) - t).abs() >= 1e-3);
            let k = eased.map_or(json!([t, v]), |x| json!([t, v, x]));
            keys.insert(keys.partition_point(|k| time(k) <= t), k);
        }
        done += 1;
        store(&mut c, &track.path, keys);
    }
    let when = from.unwrap_or(t);
    ensure!(done > 0, said("editor.no_clip_key", "the clip has no key there at %1 s", &[&when]));
    Ok(set(doc, &format!("{path}/clips/{i}"), c))
}

fn store(c: &mut Value, path: &[String], keys: Vec<Value>) {
    let (last, up) = path.split_last().expect("a track path");
    let mut at = &mut *c;
    for p in up {
        if !at[p.as_str()].is_object() {
            at[p.as_str()] = json!({});
        }
        at = &mut at[p.as_str()];
    }
    match keys.is_empty() {
        true => drop(at.as_object_mut().and_then(|m| m.remove(last))),
        false => at[last.as_str()] = json!(keys),
    }
    for depth in (0..up.len()).rev() {
        let parent = up[..depth].iter().fold(&mut *c, |v, p| &mut v[p.as_str()]);
        if parent[up[depth].as_str()].as_object().is_some_and(|m| m.is_empty()) {
            drop(parent.as_object_mut().and_then(|m| m.remove(&up[depth])));
        }
    }
}
