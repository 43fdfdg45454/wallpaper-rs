use super::{Num, Target};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sweep {
    #[default]
    Repeat,
    Mirror,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Around {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Target>,
    #[schemars(extend("x-range" = [1, 4096]))]
    pub count: u32,
    #[serde(default = "turn")]
    #[schemars(extend("x-unit" = "°"))]
    pub arc: Num,
    #[schemars(extend("x-unit" = "px"))]
    pub radius: Num,
    #[serde(default)]
    pub speed: Num,
    #[serde(default)]
    pub mode: Sweep,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub restart: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bow {
    pub amount: f32,
    #[serde(default)]
    pub side: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Taper {
    #[schemars(extend("x-range" = [0, 1]))]
    pub position: f32,
    pub speed: f32,
    #[schemars(extend("x-range" = [0, 1]))]
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Between {
    pub from: Target,
    pub to: Target,
    #[schemars(extend("x-range" = [1, 4096]))]
    pub count: u32,
    #[serde(default = "whole")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub span: Num,
    #[serde(default)]
    pub mode: Sweep,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<Bow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taper: Option<Taper>,
}

impl Default for Taper {
    fn default() -> Self {
        Taper { position: 0.5, speed: 0.0, size: 0.5 }
    }
}

fn whole() -> Num {
    Num::from(1.0)
}

fn turn() -> Num {
    Num::from(360.0)
}
