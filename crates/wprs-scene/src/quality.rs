use crate::{Kind, Scene};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    T0,
    T1,
    T2,
    T3,
}

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

impl Tier {
    pub fn detect(software: bool, discrete: bool) -> Tier {
        match (software, discrete) {
            (true, _) => Tier::T0,
            (false, true) => Tier::T3,
            (false, false) => Tier::T2,
        }
    }

    pub fn max_fps(self, on_battery: bool, wanted: u32, cost: Option<f32>) -> u32 {
        let wanted = wanted.clamp(1, 240);
        let fits = cost.is_some_and(|cost| cost * wanted as f32 <= self.budget() * 60.0);
        let cap = match (self, on_battery) {
            (Tier::T0, false) => 24,
            (Tier::T0, true) => 15,
            (Tier::T1, _) | (Tier::T2, true) => 30,
            (Tier::T2, false) if fits => wanted,
            (Tier::T2, false) | (Tier::T3, true) => 60,
            (Tier::T3, false) => wanted,
        };
        cap.min(wanted)
    }

    pub fn budget(self) -> f32 {
        [4.0, 8.0, 16.0, 40.0][self as usize]
    }

    pub fn particles(self) -> u32 {
        [2_000, 10_000, 50_000, 200_000][self as usize]
    }

    pub fn overdraw(self) -> f32 {
        [1.0, 1.5, 3.0, 6.0][self as usize]
    }

    pub fn trail(self) -> u32 {
        [0, 4, 8, 16][self as usize]
    }

    pub fn quality(self, on_battery: bool) -> Quality {
        let base = [Quality::Q0, Quality::Q2, Quality::Q3, Quality::Q3][self as usize];
        if on_battery { base.lower() } else { base }
    }
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
        let mut quality = start;
        while quality > Quality::Q0 && scene.cost(quality) > tier.budget() {
            quality = quality.lower();
        }
        quality
    }
}

impl Scene {
    pub fn cost(&self, quality: Quality) -> f32 {
        let effects = self.layers().flat_map(|l| &l.effects).filter_map(|e| {
            let d = self.effect(&e.effect)?;
            let scale = quality.effect_scale(d.class(), e.optional)? * d.scale_at(quality);
            let scale = if d.passes.len() > 1 { scale } else { 1.0 };
            Some(d.units() * scale * scale)
        });
        let particles = self.layers().filter_map(|l| match &l.kind {
            Kind::Particles { system, .. } => self.particles.get(system),
            _ => None,
        });
        let canvas = self.canvas.size;
        let particles =
            particles.map(|s| (s.capacity as f32 / 100_000.0 + s.overdraw(canvas)) * quality.particles());
        let scale = quality.render_scale();
        1.5 + (effects.sum::<f32>() + particles.sum::<f32>()) * scale * scale
    }
}
