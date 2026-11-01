use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use wprs_scene::{Bend, Body, Follows};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IkEdit {
    pub layer: String,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub tip: Option<String>,
    #[serde(default)]
    pub target: Option<Value>,
    #[serde(default)]
    pub limits: BTreeMap<String, [f32; 2]>,
    #[serde(default)]
    pub unlimit: Vec<String>,
    #[serde(default)]
    pub bend: Option<Bend>,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyEdit {
    pub layer: String,
    pub bones: Vec<String>,
    #[serde(default)]
    pub mode: Option<Body>,
    #[serde(default)]
    pub stiffness: Option<f32>,
    #[serde(default)]
    pub damping: Option<f32>,
    #[serde(default)]
    pub inertia: Option<f32>,
    #[serde(default)]
    pub gravity: Option<f32>,
    #[serde(default)]
    pub wind: Option<f32>,
    #[serde(default)]
    pub rotation: Option<[f32; 2]>,
    #[serde(default)]
    pub translation: Option<[f32; 2]>,
    #[serde(default)]
    pub rope: Vec<[f32; 2]>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindEdit {
    #[serde(default)]
    pub angle: Option<f32>,
    #[serde(default)]
    pub strength: Option<f32>,
    #[serde(default)]
    pub gusts: Option<f32>,
    #[serde(default)]
    pub off: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Spot {
    At([f32; 2]),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachEdit {
    pub layer: String,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub bone: Option<String>,
    #[serde(default)]
    pub point: Option<Spot>,
    #[serde(default)]
    pub offset: Option<[f32; 2]>,
    #[serde(default)]
    pub inherit: Option<Vec<Follows>>,
    #[serde(default)]
    pub detach: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartsEdit {
    pub layer: String,
    #[serde(default)]
    pub order: BTreeMap<String, f32>,
    #[serde(default)]
    pub hide: Vec<String>,
    #[serde(default)]
    pub show: Vec<String>,
    #[serde(default)]
    pub clip: BTreeMap<String, String>,
    #[serde(default)]
    pub unclip: Vec<String>,
}
