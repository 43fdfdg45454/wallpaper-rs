use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_FRAMES: u32 = 256;
pub const COLLECTIONS: [&str; 3] = ["ttc", "otc", "woff2"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub linear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<Wrapping>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mipmaps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 255]))]
    pub face: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-unit" = "px"))]
    pub rects: Vec<[u32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [1, 256]))]
    pub frames: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0.1, 120]))]
    pub fps: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-unit" = "s"))]
    pub durations: Vec<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub animated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<crate::Cover>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    #[default]
    Linear,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Wrapping {
    #[default]
    Clamp,
    Repeat,
    Mirror,
}

impl Asset {
    pub fn extension(&self) -> String {
        self.path.rsplit_once('.').map_or(String::new(), |(_, e)| e.to_ascii_lowercase())
    }

    pub fn moves(&self) -> bool {
        let timed = self.frames.is_some() || self.fps.is_some() || !self.durations.is_empty();
        timed || !self.rects.is_empty() || self.animated
    }

    pub fn sampled(&self) -> bool {
        self.filter.is_some() || self.wrap.is_some() || self.mipmaps.is_some()
    }

    pub fn collection(&self) -> bool {
        self.face.is_some() || COLLECTIONS.contains(&self.extension().as_str())
    }

    pub fn cells(&self) -> u32 {
        match (self.rects.len(), self.atlas) {
            (0, Some([rows, columns])) => rows.saturating_mul(columns),
            (0, None) => 1,
            (n, _) => n as u32,
        }
    }
}
