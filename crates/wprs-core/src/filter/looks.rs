use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Look {
    #[default]
    None,
    Warm,
    Cool,
    Golden,
    Vivid,
    Punch,
    Muted,
    Matte,
    Fade,
    Mono,
    Noir,
    Sepia,
    Dusk,
    Mint,
    Rose,
    Night,
}

pub const LOOKS: [Look; 16] = [
    Look::None,
    Look::Warm,
    Look::Cool,
    Look::Golden,
    Look::Vivid,
    Look::Punch,
    Look::Muted,
    Look::Matte,
    Look::Fade,
    Look::Mono,
    Look::Noir,
    Look::Sepia,
    Look::Dusk,
    Look::Mint,
    Look::Rose,
    Look::Night,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Recipe {
    pub(super) exposure: f32,
    pub(super) contrast: f32,
    pub(super) saturation: f32,
    pub(super) warmth: f32,
    pub(super) tint: f32,
    pub(super) sepia: f32,
    pub(super) lift: [f32; 3],
    pub(super) gain: [f32; 3],
}

pub(super) const NEUTRAL: Recipe = Recipe {
    exposure: 0.0,
    contrast: 1.0,
    saturation: 1.0,
    warmth: 0.0,
    tint: 0.0,
    sepia: 0.0,
    lift: [0.0; 3],
    gain: [1.0; 3],
};

const fn with(exposure: f32, contrast: f32, saturation: f32, warmth: f32, tint: f32) -> Recipe {
    Recipe { exposure, contrast, saturation, warmth, tint, ..NEUTRAL }
}

impl Look {
    pub fn name(self) -> &'static str {
        match self {
            Look::None => "none",
            Look::Warm => "warm",
            Look::Cool => "cool",
            Look::Golden => "golden",
            Look::Vivid => "vivid",
            Look::Punch => "punch",
            Look::Muted => "muted",
            Look::Matte => "matte",
            Look::Fade => "fade",
            Look::Mono => "mono",
            Look::Noir => "noir",
            Look::Sepia => "sepia",
            Look::Dusk => "dusk",
            Look::Mint => "mint",
            Look::Rose => "rose",
            Look::Night => "night",
        }
    }

    pub fn parse(name: &str) -> Option<Look> {
        LOOKS.into_iter().find(|l| l.name() == name.trim().to_ascii_lowercase())
    }

    pub(super) fn recipe(self) -> Recipe {
        match self {
            Look::None => NEUTRAL,
            Look::Warm => with(0.0, 1.0, 1.05, 0.6, 0.0),
            Look::Cool => with(0.0, 1.0, 0.95, -0.6, 0.0),
            Look::Golden => Recipe { lift: [0.03, 0.015, 0.0], ..with(0.1, 1.05, 1.1, 0.9, -0.1) },
            Look::Vivid => with(0.0, 1.1, 1.4, 0.0, 0.0),
            Look::Punch => with(0.05, 1.3, 1.15, 0.0, 0.0),
            Look::Muted => Recipe { lift: [0.03; 3], ..with(0.0, 0.92, 0.6, 0.0, 0.0) },
            Look::Matte => Recipe { lift: [0.07; 3], gain: [0.95; 3], ..with(0.0, 0.95, 0.85, 0.0, 0.0) },
            Look::Fade => Recipe { lift: [0.1; 3], gain: [0.97; 3], ..with(0.0, 0.85, 0.8, 0.1, 0.0) },
            Look::Mono => with(0.0, 1.05, 0.0, 0.0, 0.0),
            Look::Noir => Recipe { lift: [0.0; 3], gain: [0.96; 3], ..with(-0.15, 1.4, 0.0, 0.0, 0.0) },
            Look::Sepia => Recipe { sepia: 1.0, lift: [0.03, 0.02, 0.0], ..with(0.0, 0.95, 1.0, 0.0, 0.0) },
            Look::Dusk => Recipe { lift: [0.02, 0.0, 0.05], ..with(-0.1, 1.05, 0.9, -0.2, 0.5) },
            Look::Mint => Recipe { gain: [0.94, 1.02, 1.0], ..with(0.05, 0.95, 0.9, -0.3, -0.4) },
            Look::Rose => Recipe { lift: [0.04, 0.0, 0.02], ..with(0.05, 0.95, 0.9, 0.2, 0.4) },
            Look::Night => Recipe { lift: [0.0, 0.01, 0.04], ..with(-0.6, 1.1, 0.7, -0.7, 0.0) },
        }
    }
}
