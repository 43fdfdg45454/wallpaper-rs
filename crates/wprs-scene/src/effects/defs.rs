use super::{Entry, ParamKind, PassBlend, Use, full, previous};
use crate::value::Param;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamDef {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ParamKind,
    pub default: Param,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uses: Option<Use>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub label: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub help: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<Param>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassDef {
    #[serde(default)]
    pub entry: String,
    #[serde(default = "full")]
    pub scale: f32,
    #[serde(default = "previous")]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub output: Output,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<PassBlend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<[String; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    #[default]
    Next,
    Step,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cost {
    pub taps: f32,
    pub passes: u32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    pub scale: f32,
    pub settle: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tune {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Param>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub freeze: bool,
}

impl Default for Cost {
    fn default() -> Self {
        Cost { taps: 1.0, passes: 1, scale: 1.0 }
    }
}

impl Default for State {
    fn default() -> Self {
        State { scale: 1.0, settle: 0.0, fade: None }
    }
}
