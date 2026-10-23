use crate::ops::{Op, set};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use wprs_scene::{Color, Ease};

fn time(key: &Value) -> f64 {
    key[0].as_f64().unwrap_or(0.0)
}

fn mix(a: &Value, b: &Value, f: f32) -> Value {
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
    let i = keys.partition_point(|k| time(k) <= t);
    match (i.checked_sub(1).and_then(|i| keys.get(i)), keys.get(i)) {
        (Some(a), Some(b)) => {
            let ease: Ease =
                serde_json::from_value(a.get(2).cloned().unwrap_or(json!("linear"))).unwrap_or_default();
            Some(mix(&a[1], &b[1], ease.apply(((t - time(a)) / (time(b) - time(a)).max(1e-6)) as f32)))
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

pub struct Key<'a> {
    pub time: f64,
    pub value: Option<Value>,
    pub ease: Option<Value>,
    pub timeline: Option<&'a str>,
    pub remove: bool,
}

pub fn keyframe(doc: &Value, pointer: &str, mut key: Key) -> Result<Vec<Op>> {
    ensure!(key.time.is_finite() && key.time >= 0.0, "the time must be 0 or more seconds");
    key.time = (key.time * 1000.0).round() / 1000.0;
    if let Some(ease) = &key.ease {
        serde_json::from_value::<Ease>(ease.clone()).map_err(|e| anyhow!("{e}"))?;
    }
    let current = doc.pointer(pointer);
    let mut ops = match current.and_then(|c| c.get("keys")).and_then(Value::as_array) {
        Some(keys) => {
            let mut keys = keys.clone();
            let at = keys.iter().position(|k| (time(k) - key.time).abs() < 1e-3);
            if key.remove {
                let i = at.ok_or_else(|| anyhow!("{pointer} has no keyframe at {} s", key.time))?;
                let removed = keys.remove(i);
                return Ok(match keys.is_empty() {
                    true => set(doc, pointer, removed[1].clone()),
                    false => set(doc, &format!("{pointer}/keys"), Value::Array(keys)),
                });
            }
            let value = key.value.clone().or_else(|| sample(&keys, key.time)).unwrap_or(Value::Null);
            let ease = key.ease.clone().or_else(|| at.and_then(|i| keys[i].get(2).cloned()));
            let new = ease.map_or(json!([key.time, value]), |e| json!([key.time, value, e]));
            match at {
                Some(i) => keys[i] = new,
                None => keys.insert(keys.partition_point(|k| time(k) <= key.time), new),
            }
            set(doc, &format!("{pointer}/keys"), Value::Array(keys))
        }
        None => {
            ensure!(!key.remove, "{pointer} has no keyframes");
            if current.is_some_and(Value::is_object) {
                bail!("{pointer} follows another source; set a plain value first");
            }
            let value = key.value.clone().or_else(|| current.cloned());
            let value = value.ok_or_else(|| anyhow!("{pointer} has no value yet; give one"))?;
            let first = key.ease.clone().map_or(json!([key.time, value]), |e| json!([key.time, value, e]));
            set(doc, pointer, json!({"keys": [first]}))
        }
    };
    if let Some(timeline) = key.timeline {
        let mut known = doc.pointer("/scene/timelines").and_then(Value::as_array).into_iter().flatten();
        ensure!(known.any(|t| t["id"] == timeline), "there is no timeline {timeline:?}");
        let mut staged = doc.clone();
        crate::ops::apply(&mut staged, &ops)?;
        ops.extend(set(&staged, &format!("{pointer}/timeline"), json!(timeline)));
    }
    Ok(ops)
}
