use super::PageSlot;
use crate::inputs;
use crate::media::{Cover, Frame};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use wprs_core::model::Input;
use wprs_web::ToWeb;

pub(super) fn told(granted: bool, frame: Option<&Frame>, cover: Option<Option<&Cover>>) -> Value {
    let state = frame.filter(|_| granted).map(|f| f.at(Instant::now()));
    let picture = cover.map(|c| c.and_then(|c| c.pixels.image()).map(|i| png(&i)).unwrap_or_default());
    wprs_format::page::heard(state.as_ref(), picture)
}

pub(crate) fn png(image: &image::RgbaImage) -> String {
    use base64::Engine;
    let mut out = std::io::Cursor::new(Vec::new());
    match image.write_to(&mut out, image::ImageFormat::Png) {
        Ok(()) => base64::engine::general_purpose::STANDARD.encode(out.into_inner()),
        Err(_) => String::new(),
    }
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
        Some(ToWeb::Media(told(granted, frame.as_deref(), fresh.then_some(cover.as_deref()))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wprs_scene::MediaState;

    #[test]
    fn a_page_gets_the_track_with_its_colors_and_never_a_path() {
        assert_eq!(told(false, None, Some(None)), serde_json::json!({ "enabled": false, "cover": "" }));
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
