use crate::look::{Bloom, Camera, Glide, Tonemap};
use crate::value::{Value, Vec2};
use crate::{Color, schema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wprs_core::Fit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlendSpace {
    #[default]
    Linear,
    Srgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub size: [u32; 2],
    #[serde(default)]
    #[schemars(schema_with = "schema::fit")]
    pub fit: Fit,
    #[serde(default)]
    pub background: Color,
    #[serde(default)]
    pub blend_space: BlendSpace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallax: Option<Glide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloom: Option<Bloom>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hdr: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tonemap: Option<Tonemap>,
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub clear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px", "x-range" = [100, 100000]))]
    pub perspective: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px/s"))]
    pub wind: Option<Value<Vec2>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Clock {
    #[serde(with = "wprs_core::settings::fps")]
    #[schemars(schema_with = "schema::fps")]
    pub max_fps: u32,
    #[schemars(extend("x-unit" = "s"))]
    pub poster_time: f32,
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "s", "x-range" = [1, 3600]))]
    pub step: Option<f32>,
}

impl Default for Clock {
    fn default() -> Self {
        Clock { max_fps: 60, poster_time: 0.0, seed: Some(0), step: None }
    }
}

impl Canvas {
    pub fn new(size: [u32; 2]) -> Canvas {
        let (fit, background, blend_space) = (Fit::Fill, Color::default(), BlendSpace::Linear);
        let (parallax, camera, bloom, hdr, tonemap, perspective, ambient, sky, wind) = Default::default();
        Canvas {
            size,
            fit,
            background,
            blend_space,
            parallax,
            camera,
            bloom,
            hdr,
            tonemap,
            clear: true,
            perspective,
            ambient,
            sky,
            wind,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_yes(v: &bool) -> bool {
    *v
}
