use crate::action::KeyframeEdit;
use crate::blend::added;
use crate::handles::{based, end, key, reach, shaped};
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::{Color, Ease};

pub(crate) fn time_of(key: &Value) -> f64 {
    key[0].as_f64().unwrap_or(0.0)
}

pub(crate) fn mix(a: &Value, b: &Value, f: f32) -> Value {
    let lerp = |x: f64, y: f64| ((x + (y - x) * f64::from(f)) * 1e6).round() / 1e6;
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            json!(lerp(x.as_f64().unwrap_or(0.0), y.as_f64().unwrap_or(0.0)))
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).map(|(x, y)| mix(x, y, f)).collect()
        }
        (Value::String(x), Value::String(y)) => match (Color::parse(x), Color::parse(y)) {
            (Some(x), Some(y)) => {
                let c: [f32; 4] = std::array::from_fn(|i| x.0[i] + (y.0[i] - x.0[i]) * f);
                json!(Color(c).hex())
            }
            _ if f < 1.0 => a.clone(),
            _ => b.clone(),
        },
        _ if f < 1.0 => a.clone(),
        _ => b.clone(),
    }
}

pub fn sample(keys: &[Value], t: f64) -> Option<Value> {
    let i = keys.partition_point(|k| time_of(k) <= t);
    match (i.checked_sub(1).and_then(|i| keys.get(i)), keys.get(i)) {
        (Some(a), Some(b)) => {
            if let Some(v) = crate::handles::curve(a, b, t) {
                return Some(v);
            }
            let ease: Ease =
                serde_json::from_value(a.get(2).cloned().unwrap_or(json!("linear"))).unwrap_or_default();
            Some(mix(
                &a[1],
                &b[1],
                ease.apply(((t - time_of(a)) / (time_of(b) - time_of(a)).max(1e-6)) as f32),
            ))
        }
        (Some(k), None) | (None, Some(k)) => Some(k[1].clone()),
        (None, None) => None,
    }
}

pub fn value_at(value: &Value, t: f64) -> Value {
    match value.get("keys").and_then(Value::as_array) {
        Some(keys) => sample(keys, t).unwrap_or(Value::Null),
        None => value.clone(),
    }
}

pub fn keyframe(doc: &Value, e: &KeyframeEdit) -> Result<Vec<Op>> {
    let pointer = e.pointer.as_str();
    ensure!(
        e.time.is_finite() && e.time >= 0.0,
        said("editor.time_must_more", "the time must be 0 or more seconds", &[])
    );
    let time = (e.time * 1000.0).round() / 1000.0;
    if e.straighten {
        return crate::straight::straighten(doc, e, time);
    }
    if let Some(ease) = &e.ease {
        serde_json::from_value::<Ease>(ease.clone()).map_err(|e| anyhow!("{e}"))?;
    }
    shaped(e)?;
    let current = doc.pointer(pointer);
    let mut ops = match current.and_then(|c| c.get("keys")).and_then(Value::as_array) {
        Some(old) => {
            let mut keys = old.clone();
            let (mut value, mut third) = (e.value.clone(), None);
            if let Some(from) = e.from {
                let i = keys.iter().position(|k| (time_of(k) - from).abs() < 1e-3).ok_or_else(|| {
                    anyhow!(said("editor.no_keyframe_s", "%1 has no keyframe at %2 s", &[&pointer, &from]))
                })?;
                let moved = keys.remove(i);
                value = value.or_else(|| Some(moved[1].clone()));
                third = moved.get(2).cloned();
            }
            let at = keys.iter().position(|k| (time_of(k) - time).abs() < 1e-3);
            ensure!(
                e.from.is_none() || at.is_none() || e.remove,
                said("editor.already_keyframe_s", "%1 already has a keyframe at %2 s", &[&pointer, &time])
            );
            if e.remove {
                let i = at.ok_or_else(|| {
                    anyhow!(said("editor.no_keyframe_s", "%1 has no keyframe at %2 s", &[&pointer, &time]))
                })?;
                let removed = keys.remove(i);
                reach(pointer, old, &keys)?;
                let base = current.and_then(|c| c.get("base")).filter(|_| !e.absolute);
                let last = match base {
                    Some(b) => added(b, &removed[1]).ok_or_else(|| {
                        anyhow!(said(
                            "editor.base_unadded",
                            "the base of %1 does not add to its last keyframe; remove it with absolute to keep the keyframe value",
                            &[&pointer]
                        ))
                    })?,
                    None => removed[1].clone(),
                };
                return Ok(match keys.is_empty() {
                    true => set(doc, pointer, last),
                    false => set(doc, &format!("{pointer}/keys"), Value::Array(keys)),
                });
            }
            let kept = at.map(|i| keys[i][1].clone());
            let value = value.or(kept).or_else(|| sample(&keys, time)).unwrap_or(Value::Null);
            let third = third.or_else(|| at.and_then(|i| keys[i].get(2).cloned()));
            let third = end(third.as_ref(), e, &value)?;
            let new = key(time, value, third);
            match at {
                Some(i) => keys[i] = new,
                None => keys.insert(keys.partition_point(|k| time_of(k) <= time), new),
            }
            reach(pointer, old, &keys)?;
            set(doc, &format!("{pointer}/keys"), Value::Array(keys))
        }
        None => {
            ensure!(
                !e.remove && e.from.is_none(),
                said("editor.no_keyframes", "%1 has no keyframes", &[&pointer])
            );
            if current.is_some_and(Value::is_object) {
                bail!(said(
                    "editor.follows_another_source",
                    "%1 follows another source; set a plain value first",
                    &[&pointer]
                ));
            }
            let value = e.value.clone().or_else(|| current.cloned());
            let value = value.ok_or_else(|| {
                anyhow!(said("editor.no_value_yet", "%1 has no value yet; give one", &[&pointer]))
            })?;
            let third = end(None, e, &value)?;
            let new = [key(time, value, third)];
            reach(pointer, &[], &new)?;
            set(doc, pointer, json!({"keys": new}))
        }
    };
    let mut staged = doc.clone();
    crate::ops::apply(&mut staged, &ops)?;
    if let Some(timeline) = &e.timeline {
        let mut known = doc.pointer("/scene/timelines").and_then(Value::as_array).into_iter().flatten();
        ensure!(
            known.any(|t| t["id"] == timeline.as_str()),
            said(
                "editor.there_no_timeline_add",
                "there is no timeline \"%1\"; add it with the timeline action first",
                &[&timeline]
            )
        );
        ops.extend(set(&staged, &format!("{pointer}/timeline"), json!(timeline)));
        crate::ops::apply(&mut staged, &ops[ops.len() - 1..])?;
    }
    ops.extend(based(&staged, e)?);
    Ok(ops)
}
