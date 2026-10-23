use super::{Follow, System, is_zero};
use crate::validate::Check;
use crate::value::{Value, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Target {
    Point([f32; 2]),
    Follow(Follow),
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
    },
    Attractor {
        target: Target,
        strength: Value<f32>,
    },
    Vortex {
        center: Target,
        strength: Value<f32>,
    },
    Orbit {
        center: Target,
        strength: Value<f32>,
    },
    Noise(Value<f32>),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Surface {
    Canvas,
    Plane { point: [f32; 2], normal: [f32; 2] },
    Circle { center: [f32; 2], radius: f32 },
}

impl Check<'_> {
    pub(super) fn forces(&mut self, p: &str, s: &System) {
        for (k, force) in s.forces.iter().enumerate() {
            let q = format!("{p}/forces/{k}");
            match force {
                Force::Gravity(v) => self.value(&q, v),
                Force::Drag(v) | Force::Noise(v) => self.value(&q, v),
                Force::Turbulence { strength, scale } => {
                    self.value(&format!("{q}/strength"), strength);
                    self.range(&format!("{q}/scale"), *scale, 1e-6, 1.0);
                }
                Force::Attractor { target: t, strength }
                | Force::Vortex { center: t, strength }
                | Force::Orbit { center: t, strength } => {
                    self.value(&format!("{q}/strength"), strength);
                    if let Target::Point(at) = t {
                        self.place(&q, *at);
                    }
                }
            }
        }
        for (k, c) in s.collide.iter().enumerate() {
            let q = format!("{p}/collide/{k}");
            self.range(&format!("{q}/bounce"), c.bounce, 0.0, 1.0);
            self.range(&format!("{q}/friction"), c.friction, 0.0, 1.0);
            match c.bounds {
                Surface::Canvas => {}
                Surface::Plane { point, normal } => {
                    self.place(&q, point);
                    self.range(&format!("{q}/bounds/plane/normal"), normal[0].hypot(normal[1]), 1e-6, 1e6);
                }
                Surface::Circle { center, radius } => {
                    self.place(&q, center);
                    self.range(&format!("{q}/bounds/circle/radius"), radius, 1e-3, 1e5);
                }
            }
        }
    }
}
