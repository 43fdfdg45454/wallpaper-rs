use super::is_zero;
use crate::validate::Check;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stick: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Surface {
    Canvas,
    Plane {
        point: [f32; 2],
        normal: [f32; 2],
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inside: bool,
    },
    Box {
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inside: bool,
    },
    Segment {
        from: [f32; 2],
        to: [f32; 2],
    },
    Layer {
        #[schemars(extend("x-ref" = "layer"))]
        id: String,
    },
    Mask {
        #[schemars(extend("x-ref" = "image"))]
        texture: String,
        #[serde(default = "half")]
        #[schemars(extend("x-range" = [0, 1]))]
        threshold: f32,
    },
}

fn half() -> f32 {
    0.5
}

impl Check<'_> {
    pub(super) fn collider(&mut self, q: &str, c: &Collider) {
        self.range(&format!("{q}/bounce"), c.bounce, 0.0, 1.0);
        self.range(&format!("{q}/friction"), c.friction, 0.0, 1.0);
        match &c.bounds {
            Surface::Plane { point, normal } => {
                self.place(q, *point);
                self.range(&format!("{q}/bounds/plane/normal"), normal[0].hypot(normal[1]), 1e-6, 1e6);
            }
            Surface::Circle { center, radius, .. } => {
                self.place(q, *center);
                self.range(&format!("{q}/bounds/circle/radius"), *radius, 1e-3, 1e5);
            }
            _ => {}
        }
        self.collider2(q, c);
    }
}
