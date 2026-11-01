use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::AnimClip;

fn ease(k: &Value) -> Value {
    match k.get(2) {
        None => json!("linear"),
        Some(e) if e == "in_out_cubic" => json!("cubic"),
        Some(e) => e.clone(),
    }
}

fn row(kind: &str, name: &str, channel: &str, keys: &Value) -> Value {
    let keys: Vec<Value> = keys
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| json!({ "t": k[0], "value": k[1], "ease": ease(k) }))
        .collect();
    json!({ "kind": kind, "name": name, "channel": channel, "keys": keys })
}

pub fn keys(dir: &Path, doc: &Value, id: &str, clip: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, c)) = crate::clips::found(layer, clip) else { return Value::Null };
    let mut rows = Vec::new();
    for (bone, tracks) in c["bones"].as_object().into_iter().flatten() {
        for channel in ["rotation", "position", "scale"] {
            if tracks.get(channel).is_some() {
                rows.push(row("bone", bone, channel, &tracks[channel]));
            }
        }
    }
    for kind in ["shapes", "order"] {
        for (name, keys) in c[kind].as_object().into_iter().flatten() {
            rows.push(row(kind.trim_end_matches('s'), name, "", keys));
        }
    }
    let rig = crate::skin::rigged(dir, doc, layer).ok().map(|r| r.2);
    let parsed = serde_json::from_value::<AnimClip>(c.clone()).ok();
    let length = rig.as_ref().zip(parsed.as_ref()).map(|(r, p)| super::pose::length(r, p).1);
    let names = |f: fn(&wprs_scene::Rigged) -> Vec<String>| rig.as_ref().map_or(Vec::new(), f);
    json!({
        "clip": clip,
        "length": length.map(super::round),
        "loop": c.get("loop").cloned().unwrap_or(json!("repeat")),
        "rows": rows,
        "shapes": names(|r| r.shapes.clone()),
        "defaults": rig.as_ref().map_or(Vec::new(), |r| r.defaults.clone()),
        "parts": names(|r| r.parts.clone()),
    })
}
