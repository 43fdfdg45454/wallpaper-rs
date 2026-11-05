use serde_json::{Map, Value, json};
use wprs_scene::MediaState;

const SHADES: [&str; 5] = ["primary", "secondary", "tertiary", "text", "contrast"];

fn hex(c: [f32; 4]) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
}

pub fn heard(state: Option<&MediaState>, cover: Option<String>) -> Value {
    let Some(s) = state.filter(|s| s.enabled) else { return json!({ "enabled": false, "cover": "" }) };
    let present = s.present();
    let played = match (present, s.playing) {
        (false, _) => "stopped",
        (true, true) => "playing",
        (true, false) => "paused",
    };
    let colors = s.colors.map(|c| SHADES.iter().zip(c).map(|(k, v)| (k.to_string(), json!(hex(v)))));
    let colors = colors.map(|c| c.collect::<Map<_, _>>());
    let (position, duration) = if present { (s.position, s.duration) } else { (0.0, 0.0) };
    let mut told = json!({ "enabled": true, "state": played, "title": s.title, "artist": s.artist,
        "album": s.album, "colors": colors, "position": position, "duration": duration, "rate": s.rate });
    if let Some(cover) = cover {
        told["cover"] = json!(cover);
    }
    told
}
