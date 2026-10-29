use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
    Lighten,
    Darken,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    ColorDodge,
    ColorBurn,
    Hue,
    Saturation,
    Color,
    Luminosity,
    LinearBurn,
    LinearDodge,
    DarkerColor,
    LighterColor,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Subtract,
    Divide,
    Reflect,
    Glow,
    Phoenix,
    Average,
    Negation,
}

use Blend::*;

pub const BLENDS: [Blend; 32] = [
    Normal,
    Add,
    Multiply,
    Screen,
    Lighten,
    Darken,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    ColorDodge,
    ColorBurn,
    Hue,
    Saturation,
    Color,
    Luminosity,
    LinearBurn,
    LinearDodge,
    DarkerColor,
    LighterColor,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Subtract,
    Divide,
    Reflect,
    Glow,
    Phoenix,
    Average,
    Negation,
];

impl Blend {
    pub fn fixed(self) -> bool {
        matches!(self, Blend::Normal | Blend::Add | Blend::Multiply | Blend::Screen)
    }

    pub fn associative(self) -> bool {
        matches!(self, Blend::Normal | Blend::Add)
    }

    pub fn newer(self) -> bool {
        self as usize > Blend::Luminosity as usize
    }

    pub fn named(name: &str) -> Option<Blend> {
        serde_json::from_value(serde_json::Value::String(name.into())).ok()
    }
}
