use super::Layer;
use crate::content::{Gradient, Shape, Spectrum, Stroke};
use crate::figure::{MeshLayer, ModelLayer, Space};
use crate::light::Light;
use crate::look::Playback;
use crate::particles::{Instance, Render};
use crate::typeset::TextLayer;
use crate::value::Value;
use crate::{Color, Sound};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Solid {
    Flag(bool),
    Hit {
        #[schemars(extend("x-range" = [0, 1]))]
        alpha: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Group {
        layers: Vec<Layer>,
        #[serde(default)]
        clip: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        space: Option<Space>,
    },
    Image {
        #[schemars(extend("x-ref" = "image"))]
        image: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        playback: Option<Playback>,
    },
    Video {
        #[schemars(extend("x-ref" = "video"))]
        video: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        playback: Option<Playback>,
    },
    Text(Box<TextLayer>),
    Shape {
        shape: Shape,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
    },
    Particles {
        #[schemars(extend("x-ref" = "system"))]
        system: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance: Option<Box<Instance>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        render: Option<Box<Render>>,
    },
    Spectrum(Spectrum),
    Sound(Sound),
    Solid {
        color: Value<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
    },
    Light(Box<Light>),
    Adjustment {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "px"))]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        redraw: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rgb: bool,
    },
    Mesh(Box<MeshLayer>),
    Model(Box<ModelLayer>),
}

impl Default for Solid {
    fn default() -> Self {
        Solid::Flag(false)
    }
}

impl Solid {
    pub fn is_off(&self) -> bool {
        *self == Solid::Flag(false)
    }
}
