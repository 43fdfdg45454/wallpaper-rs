use crate::Color;
use crate::value::{AudioChannel, Param, Value};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Mask {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "layer"))]
    pub layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<MaskChannel>,
    pub invert: bool,
    pub feather: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaskChannel {
    R,
    G,
    B,
    A,
    Luminance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectUse {
    pub effect: String,
    #[serde(default = "shown")]
    pub enabled: Value<bool>,
    #[serde(default, deserialize_with = "mask", skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value<Param>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Rect {
        size: [f32; 2],
        #[serde(default)]
        radius: f32,
    },
    Ellipse {
        size: [f32; 2],
    },
    Ring {
        size: [f32; 2],
        width: f32,
    },
    Polygon {
        sides: u32,
        size: f32,
    },
    Star {
        points: u32,
        size: f32,
        #[serde(default = "half")]
        inner: f32,
    },
    Line {
        to: [f32; 2],
        width: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    #[serde(default)]
    pub radial: bool,
    #[serde(default)]
    pub from: [f32; 2],
    #[serde(default = "down")]
    pub to: [f32; 2],
    pub stops: Vec<(f32, Color)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    #[serde(default = "white")]
    pub color: Value<Color>,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spectrum {
    pub size: [f32; 2],
    #[serde(default)]
    pub style: SpectrumStyle,
    #[serde(default = "bands")]
    pub bands: u32,
    #[serde(default)]
    pub channel: AudioChannel,
    #[serde(default = "white")]
    pub color: Value<Color>,
    #[serde(default = "gap")]
    pub gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumStyle {
    #[default]
    Bars,
    Circle,
    Wave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

pub(crate) fn mask<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Mask>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Texture(String),
        Full(Mask),
    }
    Ok(Some(match Repr::deserialize(d)? {
        Repr::Texture(texture) => Mask { texture: Some(texture), ..Mask::default() },
        Repr::Full(mask) => mask,
    }))
}

pub(crate) fn shown() -> Value<bool> {
    Value::Fixed(true)
}

pub(crate) fn white() -> Value<Color> {
    Value::Fixed(Color([1.0; 4]))
}

fn half() -> f32 {
    0.5
}

fn down() -> [f32; 2] {
    [0.0, 1.0]
}

fn bands() -> u32 {
    32
}

fn gap() -> f32 {
    0.2
}
