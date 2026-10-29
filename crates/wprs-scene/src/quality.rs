use crate::{Scene, Tier};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Q0,
    Q1,
    Q2,
    #[default]
    Q3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Light,
    Medium,
    Heavy,
}

impl Quality {
    pub fn lower(self) -> Quality {
        [Quality::Q0, Quality::Q0, Quality::Q1, Quality::Q2][self as usize]
    }

    pub fn render_scale(self) -> f32 {
        [0.5, 0.75, 1.0, 1.0][self as usize]
    }

    pub fn max_fps(self) -> u32 {
        [20, 30, 60, 240][self as usize]
    }

    pub fn effect_scale(self, class: Class, optional: bool) -> Option<f32> {
        match (self, class) {
            (_, Class::Light) | (Quality::Q3, _) => Some(1.0),
            (Quality::Q2, _) => Some(0.5),
            (Quality::Q1, _) if optional => None,
            (Quality::Q1, Class::Medium) => Some(0.5),
            (Quality::Q1, Class::Heavy) => Some(0.25),
            _ => None,
        }
    }

    pub fn particles(self) -> f32 {
        [0.25, 0.5, 0.75, 1.0][self as usize]
    }

    pub fn trail(self, length: u32) -> u32 {
        [0, 0, length / 2, length][self as usize]
    }

    pub fn fit(scene: &Scene, tier: Tier, start: Quality) -> Quality {
        Quality::fitting(|q| scene.cost(q), tier, start)
    }

    pub fn fitting(cost: impl Fn(Quality) -> f32, tier: Tier, start: Quality) -> Quality {
        let mut quality = start;
        while quality > Quality::Q0 && cost(quality) > tier.budget() {
            quality = quality.lower();
        }
        quality
    }
}
