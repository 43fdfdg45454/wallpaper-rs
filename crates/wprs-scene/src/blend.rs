use schemars::JsonSchema;
use serde::de::IntoDeserializer;
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
    Tint,
    DiffuseLight,
}

use Blend::*;

pub const BLENDS: [Blend; 34] = [
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
    Tint,
    DiffuseLight,
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
        let words: serde::de::value::StrDeserializer<'_, serde::de::value::Error> = name.into_deserializer();
        Blend::deserialize(words).ok()
    }
}
