use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub length: u32,
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 7]))]
    pub subdivide: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_scroll: Option<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub textured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Stretching", into = "Stretching")]
pub struct Stretch {
    pub time: f32,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum Stretching {
    Time(f32),
    Range(Span),
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Span {
    #[serde(default)]
    time: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Rope {
    #[schemars(extend("x-range" = [0, 7]))]
    pub subdivide: u32,
    #[schemars(extend("x-unit" = "px"))]
    pub width: f32,
    pub uv_scale: f32,
    pub uv_scroll: f32,
    pub smooth: bool,
}

impl From<Stretching> for Stretch {
    fn from(s: Stretching) -> Self {
        match s {
            Stretching::Time(time) => Stretch { time, min: None, max: None },
            Stretching::Range(Span { time, min, max }) => Stretch { time, min, max },
        }
    }
}

impl From<Stretch> for Stretching {
    fn from(s: Stretch) -> Self {
        match (s.min, s.max) {
            (None, None) => Stretching::Time(s.time),
            (min, max) => Stretching::Range(Span { time: s.time, min, max }),
        }
    }
}

impl Stretch {
    pub(super) fn is_off(&self) -> bool {
        *self == Stretch::default()
    }
}

impl Default for Rope {
    fn default() -> Self {
        Rope { subdivide: 2, width: 4.0, uv_scale: 1.0, uv_scroll: 0.0, smooth: true }
    }
}
