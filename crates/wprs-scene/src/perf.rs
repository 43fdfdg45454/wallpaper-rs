use crate::{Effect, Layer, LayerKind, Scene, Timeline};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    Cheap,
    Medium,
    Expensive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}

impl Quality {
    pub fn scale(self) -> f32 {
        match self {
            Quality::Low => 0.25,
            Quality::Medium => 0.5,
            Quality::High => 1.0,
        }
    }
}

impl Effect {
    pub fn cost(&self) -> Cost {
        match self {
            Effect::Blur { .. } => Cost::Expensive,
            Effect::ColorGrade { .. } | Effect::ChromaticAberration { .. } => Cost::Medium,
            _ => Cost::Cheap,
        }
    }

    pub fn animates(&self) -> bool {
        match *self {
            Effect::Waves { amplitude, speed, .. }
            | Effect::Shake { amplitude, frequency: speed }
            | Effect::Pulse { amount: amplitude, speed } => amplitude != 0.0 && speed != 0.0,
            Effect::Scroll { speed_x, speed_y } => speed_x != 0.0 || speed_y != 0.0,
            _ => false,
        }
    }
}

impl Layer {
    pub fn animates(&self) -> bool {
        let moving = match &self.kind {
            LayerKind::Video { .. } => true,
            LayerKind::Particles(e) => e.rate > 0.0 && e.max_particles > 0,
            _ => false,
        };
        self.visible && self.opacity > 0.0 && (moving || self.effects.iter().any(Effect::animates))
    }
}

impl Timeline {
    pub fn moves(&self) -> bool {
        self.keyframes.windows(2).any(|w| w[0].1 != w[1].1)
    }

    pub fn layer(&self) -> Option<usize> {
        self.target.strip_prefix("/layers/")?.split('/').next()?.parse().ok()
    }
}

impl Scene {
    pub fn animated_layers(&self) -> Vec<usize> {
        let timed: Vec<usize> =
            self.timelines.iter().filter(|t| t.moves()).filter_map(Timeline::layer).collect();
        let animated = |(i, l): (usize, &Layer)| (l.animates() || timed.contains(&i)).then_some(i);
        self.layers.iter().enumerate().filter_map(animated).collect()
    }

    pub fn is_static(&self) -> bool {
        self.animated_layers().is_empty()
    }

    pub fn apply_quality(&mut self, quality: Quality) {
        let k = quality.scale();
        for layer in &mut self.layers {
            if let LayerKind::Particles(e) = &mut layer.kind {
                e.rate *= k;
                e.max_particles = (e.max_particles as f32 * k).ceil() as u32;
            }
            for effect in &mut layer.effects {
                if let Effect::Blur { radius } = effect {
                    *radius *= k;
                }
            }
        }
    }
}
