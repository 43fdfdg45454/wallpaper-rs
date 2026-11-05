use serde_json::json;
use wprs_scene::{MediaState, Scene, ScriptApi};

pub fn hears_media(scene: &Scene) -> bool {
    let hears = |s: &wprs_scene::Script| s.wants(ScriptApi::Media) || s.events.iter().any(|e| e.media());
    scene.scripts.iter().any(hears)
}

pub(crate) fn listeners(scene: &Scene) -> Vec<String> {
    let hears = |s: &&wprs_scene::Script| s.wants(ScriptApi::Media) || s.events.iter().any(|e| e.media());
    scene.scripts.iter().filter(hears).map(|s| s.path.clone()).collect()
}

pub(crate) fn snapshot(s: &MediaState, at: f64) -> String {
    if !s.enabled {
        return json!({ "enabled": false }).to_string();
    }
    let present = s.present();
    let state = match (present, s.playing) {
        (false, _) => "stopped",
        (true, true) => "playing",
        (true, false) => "paused",
    };
    let number = |v: f32| if present && v.is_finite() { f64::from(v) } else { 0.0 };
    json!({ "enabled": true, "state": state, "track": s.track, "cover": s.cover, "has_cover": s.cover != 0,
        "colors": s.colors, "title": s.title, "artist": s.artist, "album": s.album,
        "album_artist": s.album_artist, "subtitle": s.subtitle, "genres": s.genres, "kind": s.kind,
        "position": number(s.position), "duration": number(s.duration),
        "rate": if s.rate.is_finite() { f64::from(s.rate) } else { 1.0 }, "at": at })
    .to_string()
}
