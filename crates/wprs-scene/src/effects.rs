use crate::Quality;
use crate::value::Param;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

pub const FRAME_WGSL: &str = include_str!("frame.wgsl");
pub const RELIEF_WGSL: &str = include_str!("relief.wgsl");
pub const PRELUDE: &str = concat!(
    include_str!("frame.wgsl"),
    include_str!("prelude.wgsl"),
    include_str!("helpers.wgsl"),
    include_str!("relief.wgsl")
);
pub const SLOTS: usize = 16;
const FILES: &[(&str, &str, &str)] = &include!(concat!(env!("OUT_DIR"), "/effects.rs"));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub name: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<Use>,
    #[serde(default)]
    pub mask_mode: MaskMode,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    #[serde(default)]
    pub passes: Vec<PassDef>,
    #[serde(default)]
    pub cost: Cost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuse: Option<Fuse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solve: Option<Solve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pace: Option<Pace>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quality: BTreeMap<Quality, Tune>,
    #[serde(skip)]
    pub wgsl: String,
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
}

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassDef {
    pub entry: String,
    #[serde(default = "full")]
    pub scale: f32,
    #[serde(default = "previous")]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub output: Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    #[default]
    Next,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cost {
    pub taps: f32,
    pub passes: u32,
    pub scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    pub scale: f32,
    pub settle: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tune {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Param>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

impl Default for Cost {
    fn default() -> Self {
        Cost { taps: 1.0, passes: 1, scale: 1.0 }
    }
}

impl Default for State {
    fn default() -> Self {
        State { scale: 1.0, settle: 0.0 }
    }
}

static EXTRA: RwLock<Vec<&'static Descriptor>> = RwLock::new(Vec::new());

pub fn builtin() -> &'static [Descriptor] {
    static BUILTIN: OnceLock<Vec<Descriptor>> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        let parse = |(id, json, wgsl): &(&str, &str, &str)| match Descriptor::parse(json, wgsl) {
            Ok(d) if d.id == *id => d,
            Ok(d) => panic!("effects/{id}.json declares the id {}", d.id),
            Err(e) => panic!("effects/{id}.json: {e}"),
        };
        FILES.iter().map(parse).collect()
    })
}

pub fn descriptor(id: &str) -> Option<&'static Descriptor> {
    let extra = || EXTRA.read().unwrap_or_else(|e| e.into_inner()).iter().copied().find(|d| d.id == id);
    builtin().iter().find(|d| d.id == id).or_else(extra)
}

pub fn register(d: Descriptor) -> Result<&'static Descriptor, String> {
    let mut extra = EXTRA.write().unwrap_or_else(|e| e.into_inner());
    let known = builtin().iter().chain(extra.iter().copied()).find(|k| k.id == d.id);
    if let Some(known) = known {
        return (*known == d).then_some(known).ok_or_else(|| format!("{} is already defined", d.id));
    }
    if let Some(problem) = d.problems().into_iter().next() {
        return Err(problem);
    }
    let d: &'static Descriptor = Box::leak(Box::new(d));
    extra.push(d);
    Ok(d)
}

fn full() -> f32 {
    1.0
}

fn previous() -> Vec<String> {
    vec!["previous".into()]
}
