use anyhow::{Result, anyhow};
use serde_json::{Map, Value};
use wprs_core::message::said;

const SPREADS: [&str; 10] =
    ["life", "speed", "angle", "size", "rotation", "spin", "frame", "color", "alpha", "radial"];

pub(super) fn kept(init: &Map<String, Value>, key: &str, given: &Value) -> Result<Value> {
    let Some(list) = given.as_array().filter(|_| SPREADS.contains(&key)) else {
        return Ok(given.clone());
    };
    let old = init.get(key).and_then(Value::as_array);
    let mut out = Vec::new();
    for (i, v) in list.iter().enumerate() {
        let kept = old.and_then(|o| o.get(i)).cloned();
        match (v, i) {
            (Value::Null, 2) => out.extend(kept),
            (Value::Null, _) => out.push(kept.ok_or_else(|| {
                anyhow!(said("editor.no_end_kept", "%1 has no end to keep yet; give both ends", &[&key]))
            })?),
            _ => out.push(v.clone()),
        }
    }
    if out.get(2).and_then(Value::as_f64).is_some_and(|c| (c - 1.0).abs() < 1e-3) {
        out.truncate(2);
    }
    Ok(Value::Array(out))
}
