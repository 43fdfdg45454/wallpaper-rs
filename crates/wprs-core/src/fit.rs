use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
    Span,
}

impl Fit {
    pub const ALL: [Fit; 6] = [Fit::Fill, Fit::Fit, Fit::Stretch, Fit::Center, Fit::Tile, Fit::Span];

    pub fn as_str(self) -> &'static str {
        match self {
            Fit::Fill => "fill",
            Fit::Fit => "fit",
            Fit::Stretch => "stretch",
            Fit::Center => "center",
            Fit::Tile => "tile",
            Fit::Span => "span",
        }
    }
}

impl fmt::Display for Fit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown fit mode '{0}' (expected fill, fit, stretch, center, tile or span)")]
pub struct UnknownFit(pub String);

impl FromStr for Fit {
    type Err = UnknownFit;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Fit::ALL
            .into_iter()
            .find(|f| f.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| UnknownFit(s.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub repeat: bool,
}

impl UvRect {
    pub fn compute(fit: Fit, image: (u32, u32), target: (u32, u32)) -> UvRect {
        let [iw, ih, tw, th] = [image.0, image.1, target.0, target.1].map(|v| v.max(1) as f32);
        let cover = (tw / iw).max(th / ih);
        let contain = (tw / iw).min(th / ih);
        let (shown_w, shown_h) = match fit {
            Fit::Stretch => (tw, th),
            Fit::Fill | Fit::Span => (iw * cover, ih * cover),
            Fit::Fit => (iw * contain, ih * contain),
            Fit::Center | Fit::Tile => (iw, ih),
        };
        let scale = [tw / shown_w, th / shown_h];
        let repeat = fit == Fit::Tile;
        let offset = if repeat { [0.0, 0.0] } else { [0.5 - 0.5 * scale[0], 0.5 - 0.5 * scale[1]] };
        UvRect { offset, scale, repeat }
    }

    pub fn framed(self, zoom: f32, pan: [f32; 2]) -> UvRect {
        let scale = self.scale.map(|s| s / zoom.clamp(1.0, 8.0));
        let offset = [0, 1].map(|i| {
            let room = ((1.0 - scale[i]) / 2.0).max(0.0);
            0.5 + pan[i].clamp(-1.0, 1.0) * room - scale[i] / 2.0
        });
        UvRect { offset, scale, repeat: self.repeat }
    }
}
