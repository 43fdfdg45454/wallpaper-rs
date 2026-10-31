use super::full;
use crate::value::Param;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Layer,
    Material,
    Particle,
}

impl Stage {
    pub fn is_layer(&self) -> bool {
        *self == Stage::Layer
    }

    pub fn function(self) -> Option<(&'static str, &'static str)> {
        match self {
            Stage::Layer => None,
            Stage::Material => Some(("material", "Surface")),
            Stage::Particle => Some(("particle", "Sprite")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderTarget {
    #[serde(default = "full")]
    pub scale: f32,
    #[serde(default)]
    pub format: Texel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Texel {
    #[default]
    Rgba8,
    Rgba16f,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub id: String,
    pub values: Vec<Param>,
    pub default: Param,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_bound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub value: f32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub label: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassBlend {
    Replace,
    Normal,
    Add,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Use {
    Time,
    Cursor,
    Clicks,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskMode {
    #[default]
    Mix,
    Strength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fuse {
    Uv,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Solve {
    Homography,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pace {
    Rate(String),
    Sweep([String; 3]),
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Float,
    Vec2,
    Color,
    Bool,
    Texture,
    Lut,
    Int,
    Enum,
    Blend,
    Position,
    Mask,
}
