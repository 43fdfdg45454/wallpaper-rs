use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_CHILDREN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    Static,
    Spawn,
    Death,
    Follow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Inheritable {
    Color,
    Size,
    Alpha,
    Velocity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Child {
    #[schemars(extend("x-ref" = "system"))]
    pub system: String,
    pub on: Moment,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub probability: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub offset: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°"))]
    pub angle: Option<f32>,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub scale: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub points_from: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inherit: Vec<Inheritable>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub follow_inherit: bool,
}

fn one() -> f32 {
    1.0
}
