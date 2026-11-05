use super::PageSlot;
use crate::inputs;
use crate::media::{Cover, Frame};
use base64::Engine;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Instant;
use wprs_core::model::Input;
use wprs_web::ToWeb;

const SHADES: [&str; 5] = ["primary", "secondary", "tertiary", "text", "contrast"];

fn hex(c: [f32; 4]) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
}

fn png(cover: &Cover) -> String {
    let mut out = std::io::Cursor::new(Vec::new());
    let written = cover.pixels.image().map(|i| i.write_to(&mut out, image::ImageFormat::Png).is_ok());
    match written {
        Some(true) => base64::engine::general_purpose::STANDARD.encode(out.into_inner()),
        _ => String::new(),
    }
}

pub(super) fn told(granted: bool, frame: Option<&Frame>, cover: Option<String>) -> Value {
    let Some(f) = frame.filter(|_| granted) else { return json!({ "enabled": false, "cover": "" }) };
    let s = f.at(Instant::now());
    let present = s.present();
    let state = match (present, s.playing) {
        (false, _) => "stopped",
        (true, true) => "playing",
        (true, false) => "paused",
    };
    let colors = s.colors.map(|c| {
        SHADES.iter().zip(c).map(|(k, v)| (k.to_string(), json!(hex(v)))).collect::<serde_json::Map<_, _>>()
    });
    let mut told = json!({ "enabled": true, "state": state, "title": s.title, "artist": s.artist, "album": s.album,
        "colors": colors, "position": if present { s.position } else { 0.0 }, "duration": if present { s.duration } else { 0.0 },
        "rate": s.rate });
    if let Some(cover) = cover {
        told["cover"] = json!(cover);
    }
    told
}

impl PageSlot {
    pub(super) fn hear(&mut self, names: &[String]) -> Option<ToWeb> {
        if !self.spec.pack.uses[4] {
            return None;
        }
        let granted = names.iter().any(|n| inputs::allows(n, Input::Media) || crate::media::simulated(n));
        let frame: Option<Arc<Frame>> = names.iter().find_map(|n| crate::media::of(n)).filter(|_| granted);
        let now = (granted, frame.as_ref().map(|f| f.serial));
        if self.media == Some(now) {
            return None;
        }
        self.media = Some(now);
        let cover = frame.as_ref().and_then(|f| f.cover.clone());
        let serial = cover.as_ref().map_or(0, |c| c.serial);
        let fresh = std::mem::replace(&mut self.cover, serial) != serial;
        let sent = fresh.then(|| cover.as_deref().map(png).unwrap_or_default());
        Some(ToWeb::Media(told(granted, frame.as_deref(), sent)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wprs_scene::MediaState;

    #[test]
    fn a_page_gets_the_track_with_its_colors_and_never_a_path() {
        assert_eq!(told(false, None, Some(String::new())), json!({ "enabled": false, "cover": "" }));
        let colors = Some([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0; 4],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let state = MediaState {
            enabled: true,
            track: 1,
            title: "Night Drive".into(),
            artist: "Example Artist".into(),
            playing: false,
            position: 30.0,
            duration: 200.0,
            rate: 1.0,
            colors,
            ..MediaState::default()
        };
        let frame = Frame::new(state);
        let shown = told(true, Some(&frame), None);
        assert_eq!(shown["state"], "paused");
        assert_eq!(shown["colors"]["primary"], "#ff0000");
        assert_eq!((shown["position"].as_f64(), shown["duration"].as_f64()), (Some(30.0), Some(200.0)));
        assert!(shown.get("cover").is_none(), "an unchanged cover is not sent again");
        assert!(!shown.to_string().contains('/'), "{shown}");
        assert_eq!(
            told(false, Some(&frame), None)["enabled"],
            false,
            "without the grant nothing of the track"
        );
    }
}
