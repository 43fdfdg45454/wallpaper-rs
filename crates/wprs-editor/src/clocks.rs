use crate::keys::value_at;
use serde_json::{Value, json};
use std::path::Path;

fn videos(layer: &Value) -> Vec<&str> {
    match &layer["video"] {
        Value::String(v) => vec![v.as_str()],
        v => v["map"].as_object().map(|m| m.values().filter_map(Value::as_str).collect()).unwrap_or_default(),
    }
}

pub fn position(start: f64, end: f64, rate: f64, repeat: bool, time: f64) -> f64 {
    let ran = time.max(0.0) * rate.max(0.0);
    let span = end - start;
    match () {
        _ if !span.is_finite() || span <= 0.0 => start + ran,
        _ if repeat => start + ran % span,
        _ => (start + ran).min(end),
    }
}

pub fn clocks(dir: &Path, doc: &Value, time: f64, strip: usize) -> Value {
    let assets = &doc["scene"]["assets"];
    let own = crate::layers::walk(doc).into_iter().filter(|(_, l)| l["type"] == "video");
    let own = own.filter(|(_, l)| l["playback"]["own_clock"] == true);
    let rows = own.map(|(pointer, l)| {
        let p = &l["playback"];
        let names = videos(l);
        let files: Vec<&str> = names.iter().filter_map(|v| assets[*v]["path"].as_str()).collect();
        let frames = files.first().filter(|_| strip > 0).map(|f| crate::strips::strip(&dir.join(f), strip));
        let files = files.into_iter();
        let length = files.filter_map(|f| wprs_scene::film_file(&dir.join(f))).fold(0.0f32, f32::max);
        let start = p["start"].as_f64().unwrap_or(0.0);
        let end = p["end"].as_f64().or((length > 0.0).then_some(f64::from(length)));
        let rate = value_at(&p["rate"], time).as_f64().unwrap_or(1.0);
        let repeat = p["loop"].as_bool().unwrap_or(true);
        let playing = value_at(&p["playing"], time).as_bool().unwrap_or(true);
        let at =
            position(start, end.unwrap_or(f64::INFINITY), rate, repeat, if playing { time } else { 0.0 });
        json!({ "layer": l["id"], "pointer": format!("{pointer}/playback"), "videos": names,
            "length": (length > 0.0).then_some(length), "start": start, "end": end, "rate": rate,
            "loop": repeat, "playing": playing, "at": at, "keyed": p["rate"].get("keys").is_some(), "strip": frames })
    });
    json!({ "time": time, "clocks": rows.collect::<Vec<_>>() })
}
