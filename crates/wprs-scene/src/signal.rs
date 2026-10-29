use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CLOCKS: [&str; 6] = ["day", "hour", "minute", "second", "weekday", "month"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reduce {
    #[default]
    Mean,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Smooth {
    Both(f32),
    Apart { up: f32, down: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Gain {
    #[default]
    Auto,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Hearing {
    #[schemars(extend("x-unit" = "s", "x-range" = [0.001, 2]))]
    pub attack: f32,
    #[schemars(extend("x-unit" = "s", "x-range" = [0.01, 10]))]
    pub release: f32,
    pub gain: Gain,
}

impl Default for Hearing {
    fn default() -> Self {
        Hearing { attack: 0.02, release: 0.2, gain: Gain::Auto }
    }
}

impl Smooth {
    pub fn seconds(self) -> Option<f32> {
        match self {
            Smooth::Both(tau) if tau > 0.0 => Some(tau),
            _ => None,
        }
    }

    pub fn times(self) -> [f32; 2] {
        match self {
            Smooth::Both(tau) => [tau; 2],
            Smooth::Apart { up, down } => [up, down],
        }
    }
}
