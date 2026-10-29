use super::seed::{Carry, Hsv, OffsetNoise, Palette, Remap, Turbulent};
use super::{EmitShape, Periodic, Spread, is_zero};
use crate::Color;
use crate::value::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Emitter {
    pub shape: EmitShape,
    #[serde(default = "zero")]
    pub rate: Value<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<Burst>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<Follow>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub inherit: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "s"))]
    pub duration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "s"))]
    pub delay: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periodic: Option<Periodic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_step: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub audio_gate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cue {
    Beat,
    Click,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Cue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Init {
    pub life: Spread<f32>,
    pub speed: Spread<f32>,
    pub angle: Spread<f32>,
    pub size: Spread<f32>,
    pub rotation: Spread<f32>,
    pub spin: Spread<f32>,
    pub frame: Spread<f32>,
    pub color: Spread<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub alpha: Option<Spread<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hsv: Option<Hsv>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<Palette>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px/s"))]
    pub velocity: Option<Spread<[f32; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px/s"))]
    pub radial: Option<Spread<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherit: Option<Carry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbulent: Option<Turbulent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_noise: Option<OffsetNoise>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub depth: Option<Spread<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px/s"))]
    pub depth_speed: Option<Spread<f32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remap: Vec<Remap>,
}

impl Default for Init {
    fn default() -> Self {
        let white = Color([1.0; 4]);
        let [rotation, spin, frame] = [[0.0; 2]; 3].map(Spread::from);
        let [life, speed, angle, size] =
            [[1.0, 2.0], [10.0, 40.0], [0.0, 360.0], [2.0, 6.0]].map(Spread::from);
        let (alpha, hsv, palette, velocity, radial, inherit, turbulent, offset_noise) = Default::default();
        let (depth, depth_speed, remap) = Default::default();
        Init {
            life,
            speed,
            angle,
            size,
            rotation,
            spin,
            frame,
            color: Spread::from([white, white]),
            alpha,
            hsv,
            palette,
            velocity,
            radial,
            inherit,
            turbulent,
            offset_noise,
            depth,
            depth_speed,
            remap,
        }
    }
}

fn zero() -> Value<f32> {
    Value::Fixed(0.0)
}
