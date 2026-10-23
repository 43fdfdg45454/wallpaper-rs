use super::is_zero;
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EmitShape {
    Point([f32; 2]),
    Box([[f32; 2]; 2]),
    Circle {
        center: [f32; 2],
        radius: f32,
        #[serde(default)]
        inner: f32,
    },
    Line([[f32; 2]; 2]),
    Edge,
    Mask {
        #[schemars(extend("x-ref" = "image"))]
        texture: String,
        #[serde(default = "half")]
        threshold: f32,
    },
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
    pub life: [f32; 2],
    pub speed: [f32; 2],
    pub angle: [f32; 2],
    pub size: [f32; 2],
    pub rotation: [f32; 2],
    pub spin: [f32; 2],
    pub frame: [f32; 2],
    pub color: [Color; 2],
}

impl Default for Init {
    fn default() -> Self {
        let white = Color([1.0; 4]);
        let (rotation, spin, frame) = ([0.0; 2], [0.0; 2], [0.0; 2]);
        let (life, speed, angle, size) = ([1.0, 2.0], [10.0, 40.0], [0.0, 360.0], [2.0, 6.0]);
        Init { life, speed, angle, size, rotation, spin, frame, color: [white, white] }
    }
}

fn zero() -> Value<f32> {
    Value::Fixed(0.0)
}

fn half() -> f32 {
    0.5
}
