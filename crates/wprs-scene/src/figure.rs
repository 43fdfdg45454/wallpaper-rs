use crate::rig::Rig;
use crate::value::{Param, Value, Vec3};
use crate::{Color, Layer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MATERIALS: [&str; 5] = ["pbr", "unlit", "fur", "foliage", "chroma"];
pub const MAX_FIGURES: usize = 16;
pub const MAX_SHADOWS: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshLayer {
    #[schemars(extend("x-ref" = "model"))]
    pub mesh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<Value<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub size: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Value<Color>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<String, MeshPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<MeshDepth>,
    #[serde(flatten)]
    pub rig: Rig,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MeshPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [-1000, 1000]))]
    pub order: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<Value<bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MeshDepth {
    #[schemars(extend("x-range" = [0, 10]))]
    pub scale: f32,
    pub cull: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelLayer {
    #[schemars(extend("x-ref" = "model"))]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub size: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Value<Vec3>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°"))]
    pub rotation: Option<Value<Vec3>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Value<Vec3>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub materials: BTreeMap<String, Material>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub root_motion: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub env: Option<String>,
    #[serde(flatten)]
    pub rig: Rig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cull {
    Back,
    Front,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Opaque,
    Mask,
    Blend,
    Add,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Material {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "material"))]
    pub shader: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Value<Color>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emissive: Option<Value<Color>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_sided: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cast_shadows: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receive_shadows: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cull: Option<Cull>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_test: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_write: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend: Option<Coverage>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value<Param>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Space {
    #[default]
    #[serde(rename = "2d")]
    Flat,
    #[serde(rename = "3d")]
    Depth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follows {
    Position,
    Rotation,
    Scale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attach {
    #[schemars(extend("x-ref" = "layer"))]
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub offset: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherit: Option<Vec<Follows>>,
}

impl Default for MeshDepth {
    fn default() -> Self {
        MeshDepth { scale: 1.0, cull: false }
    }
}

impl Layer {
    pub fn rig(&self) -> Option<&Rig> {
        match &self.kind {
            crate::Kind::Mesh(m) => Some(&m.rig),
            crate::Kind::Model(m) => Some(&m.rig),
            _ => None,
        }
    }

    pub fn figure(&self) -> Option<&str> {
        match &self.kind {
            crate::Kind::Mesh(m) => Some(&m.mesh),
            crate::Kind::Model(m) => Some(&m.model),
            _ => None,
        }
    }
}
