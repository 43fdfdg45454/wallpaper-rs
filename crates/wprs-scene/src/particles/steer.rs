use super::Target;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ring {
    #[schemars(extend("x-unit" = "px"))]
    pub radius: f32,
    #[schemars(extend("x-unit" = "px"))]
    pub width: f32,
    pub pull: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Wobble {
    Position,
    Alpha,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Oscillate {
    pub what: Wobble,
    #[schemars(extend("x-unit" = "Hz"))]
    pub freq: [f32; 2],
    pub scale: [f32; 2],
    #[serde(default)]
    pub phase: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlowNear {
    pub target: Target,
    #[schemars(extend("x-unit" = "px"))]
    pub inner: f32,
    #[schemars(extend("x-unit" = "px"))]
    pub outer: f32,
    #[schemars(extend("x-range" = [0, 1]))]
    pub factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeepDistance {
    pub target: Target,
    #[schemars(extend("x-unit" = "px"))]
    pub distance: f32,
    pub strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeepBetween {
    pub from: Target,
    pub to: Target,
    pub strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Boids {
    #[schemars(extend("x-unit" = "px"))]
    pub radius: f32,
    pub separation: f32,
    pub alignment: f32,
    pub cohesion: f32,
    #[schemars(extend("x-unit" = "px/s"))]
    pub max_speed: f32,
}
