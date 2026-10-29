use super::seed::Remap;
use super::{Follow, Handle, is_zero};
use crate::value::{Value, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Target {
    Point([f32; 2]),
    Follow(Follow),
    Control(Handle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Falloff {
    Constant,
    Linear,
    InverseSquare,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Force {
    #[serde(alias = "wind")]
    Gravity(Value<Vec2>),
    Drag(Value<f32>),
    Turbulence {
        strength: Value<f32>,
        scale: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_scale: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mask: Option<[f32; 2]>,
    },
    Attractor {
        target: Target,
        strength: Value<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "px"))]
        radius: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        falloff: Option<Falloff>,
    },
    Vortex {
        center: Target,
        strength: Value<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "px"))]
        radius: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        falloff: Option<Falloff>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inner: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outer: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ring: Option<Ring>,
    },
    Orbit {
        center: Target,
        strength: Value<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "px"))]
        radius: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        falloff: Option<Falloff>,
    },
    Noise(Value<f32>),
    Oscillate(Oscillate),
    Cap(Value<f32>),
    Torque(Value<f32>),
    SpinDrag(Value<f32>),
    SlowNear(SlowNear),
    KeepDistance(KeepDistance),
    KeepBetween(KeepBetween),
    Boids(Boids),
    Remap(Remap),
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Collider {
    pub bounds: Surface,
    #[serde(default)]
    pub bounce: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub friction: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub kill: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stick: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Surface {
    Canvas,
    Plane {
        point: [f32; 2],
        normal: [f32; 2],
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inside: bool,
    },
    Box {
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inside: bool,
    },
    Segment {
        from: [f32; 2],
        to: [f32; 2],
    },
    Layer {
        #[schemars(extend("x-ref" = "layer"))]
        id: String,
    },
}
