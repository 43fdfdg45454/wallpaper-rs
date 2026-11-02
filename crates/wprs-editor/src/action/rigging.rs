use super::names::{half, tenth, whole};
use crate::skin::Mode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wprs_scene::Loop;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyEase {
    Step,
    Linear,
    Cubic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshCut {
    pub image: String,
    #[serde(default)]
    pub outline: bool,
    #[serde(default)]
    pub cells: Option<u32>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoneEdit {
    pub layer: String,
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub at: Option<[f32; 2]>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepthStroke {
    pub layer: String,
    pub points: Vec<[f32; 2]>,
    pub radius: f32,
    pub by: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightStroke {
    pub layer: String,
    pub bone: String,
    pub points: Vec<[f32; 2]>,
    pub mode: Mode,
    pub by: f32,
    #[serde(default = "tenth")]
    pub radius: f32,
    #[serde(default = "half")]
    pub falloff: f32,
    #[serde(default)]
    pub lock: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoWeights {
    pub layer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipEdit {
    pub layer: String,
    pub id: String,
    #[serde(default)]
    pub add: bool,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub length: Option<f32>,
    #[serde(default, rename = "loop")]
    pub repeat: Option<Loop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipKey {
    pub layer: String,
    pub clip: String,
    pub time: f64,
    #[serde(default)]
    pub bone: Option<String>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub position: Option<[f32; 2]>,
    #[serde(default)]
    pub scale: Option<[f32; 2]>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub value: Option<f32>,
    #[serde(default)]
    pub ease: Option<KeyEase>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub from: Option<f64>,
    #[serde(default)]
    pub copy: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionEdit {
    pub layer: String,
    pub name: String,
    #[serde(default)]
    pub shapes: BTreeMap<String, f32>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskStroke {
    pub pointer: String,
    #[serde(default)]
    pub channel: Option<usize>,
    pub radius: f32,
    #[serde(default = "half")]
    pub hardness: f32,
    #[serde(default = "whole")]
    pub value: f32,
    pub points: Vec<[f32; 2]>,
}
