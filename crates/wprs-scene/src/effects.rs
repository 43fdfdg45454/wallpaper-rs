use crate::Quality;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

mod defs;
mod parts;

pub use defs::{Cost, Output, ParamDef, PassDef, State, Tune};
pub use parts::{
    Entry, Fuse, MaskMode, Pace, ParamKind, PassBlend, RenderTarget, Solve, Stage, Texel, Use, Variant,
};

pub const FRAME_WGSL: &str = include_str!("frame.wgsl");
pub const NOISE_WGSL: &str = include_str!("noise.wgsl");
pub const RELIEF_WGSL: &str = include_str!("relief.wgsl");
pub const BLEND_WGSL: &str = include_str!("blend.wgsl");
pub const PRELUDE: &str = concat!(
    include_str!("frame.wgsl"),
    include_str!("prelude.wgsl"),
    include_str!("helpers.wgsl"),
    include_str!("oklab.wgsl"),
    include_str!("noise.wgsl"),
    include_str!("dither.wgsl"),
    include_str!("relief.wgsl"),
    include_str!("blend.wgsl")
);
pub const SLOTS: usize = 16;
pub const GENERIC_PICTURES: [&str; 4] = ["white", "black", "normal_flat", "noise"];
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
    pub min_fps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solve: Option<Solve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pace: Option<Pace>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quality: BTreeMap<Quality, Tune>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub targets: BTreeMap<String, RenderTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Variant>,
    #[serde(default, skip_serializing_if = "Stage::is_layer")]
    pub stage: Stage,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub compose: bool,
    #[serde(skip)]
    pub wgsl: String,
    #[serde(skip)]
    pub key: u64,
    #[serde(skip)]
    pub reads: f32,
    #[serde(skip)]
    pub ops: f32,
    #[serde(skip)]
    pub size: u64,
}

impl Descriptor {
    pub fn below(&self) -> bool {
        self.compose || self.passes.iter().any(|p| p.inputs.iter().any(|i| i == "backdrop"))
    }
}

pub(crate) static EXTRA: RwLock<Vec<&'static Descriptor>> = RwLock::new(Vec::new());

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
