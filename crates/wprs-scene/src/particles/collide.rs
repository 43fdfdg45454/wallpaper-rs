use super::is_zero;
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
