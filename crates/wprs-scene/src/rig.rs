use crate::timeline::{Event, Loop};
use crate::value::{Value, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod tracks;

pub use tracks::BoneKeys;

pub const MAX_CLIPS_PER_MODEL: usize = 64;
pub const MAX_ANIMATION_LAYERS: usize = 16;
pub const MAX_CHAINS: usize = 16;
pub const MAX_CHAIN: usize = 8;
pub const MAX_BODIES: usize = 64;
pub const MAX_EXPRESSIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Rig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<AnimClip>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<AnimLayer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ik: Vec<IkChain>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub physics: Vec<BonePhysics>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<BoneRule>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub shapes: BTreeMap<String, Value<f32>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub expressions: BTreeMap<String, BTreeMap<String, f32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hit: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimClip {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default)]
    #[schemars(extend("x-unit" = "s"))]
    pub from: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "s"))]
    pub to: Option<f32>,
    #[serde(default = "repeat", rename = "loop")]
    pub repeat: Loop,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bones: BTreeMap<String, BoneKeys>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shapes: BTreeMap<String, Vec<crate::Key<f32>>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub order: BTreeMap<String, Vec<crate::Key<f32>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimLayer {
    #[schemars(extend("x-ref" = "clip"))]
    pub clip: String,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub weight: Value<f32>,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 16]))]
    pub rate: Value<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub additive: bool,
    #[serde(default)]
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 10]))]
    pub blend_in: f32,
    #[serde(default)]
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 10]))]
    pub blend_out: f32,
    #[serde(default = "crate::content::shown")]
    pub enabled: Value<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Reach {
    Point(Value<Vec2>),
    #[schemars(extend("pattern" = "^(cursor|bone:.+|layer:[a-z0-9_]{1,32})$"))]
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Bend {
    #[default]
    Clockwise,
    Counterclockwise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IkChain {
    pub chain: Vec<String>,
    pub target: Reach,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(extend("x-unit" = "°"))]
    pub limits: BTreeMap<String, [f32; 2]>,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub weight: Value<f32>,
    #[serde(default)]
    pub bend: Bend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    #[default]
    Spring,
    Rigid,
    Rope,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BonePhysics {
    pub bone: String,
    #[serde(default)]
    pub mode: Body,
    #[serde(default = "half")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub stiffness: f32,
    #[serde(default = "half")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub damping: f32,
    #[serde(default = "half")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub inertia: f32,
    #[serde(default)]
    #[schemars(extend("x-range" = [-10, 10]))]
    pub gravity: f32,
    #[serde(default)]
    #[schemars(extend("x-range" = [0, 1]))]
    pub wind: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°"))]
    pub rotation: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub translation: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoneRule {
    pub bone: String,
    pub parent: String,
    #[schemars(extend("x-range" = [0, 1]))]
    pub weight: Value<f32>,
}

fn one() -> Value<f32> {
    Value::Fixed(1.0)
}

fn half() -> f32 {
    0.5
}

fn repeat() -> Loop {
    Loop::Repeat
}

impl Rig {
    pub fn is_empty(&self) -> bool {
        *self == Rig::default()
    }
}
