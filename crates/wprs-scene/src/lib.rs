mod affine;
mod animate;
mod chain;
mod content;
mod cube;
mod easing;
mod effects;
mod eval;
mod form;
mod fx;
mod input;
mod layer;
mod particles;
mod props;
mod quality;
mod resolve;
mod rules;
mod runtime;
mod timeline;
mod uses;
mod validate;
mod value;
mod wake;

pub use affine::Affine;
pub use animate::Animate;
pub use chain::{CHAIN_ENTRY, effect_source, fusion};
pub use content::{Align, EffectUse, Gradient, Mask, MaskChannel, Shape, Spectrum, SpectrumStyle, Stroke};
pub use cube::{Lut, LutError, MAX_LUT_BYTES, MAX_LUT_SIZE, MAX_LUT_TOTAL};
pub use easing::Ease;
pub use effects::{
    Cost, Descriptor, FRAME_WGSL, Fuse, MaskMode, Output, PRELUDE, Pace, ParamDef, ParamKind, PassDef,
    RELIEF_WGSL, SLOTS, Solve, State, Tune, Use, builtin, descriptor, register,
};
pub use eval::{Content, Draw, EffectDraw, Frame};
pub use form::form;
pub use input::{AudioFrame, BANDS, LayerOverride, PointerState, ScriptOutputs, TimelineCommand};
pub use layer::{Anchor, Blend, Kind, Layer, Transform};
pub use particles::{
    Bake, Burst, CPU_PARTICLES, Census, Cloud, Collider, Cue, EmitShape, Emitter, Follow, Force, Frames,
    Init, MAX_COLLIDERS, MAX_CURVE, MAX_EMITTERS, MAX_FORCES, MAX_POINTS, MAX_PREWARM, Particle, Render,
    STEP, SUBSTEPS, Sim, Surface, Swarm, System, Target, Trail,
};
pub use props::{Choice, Control, Preset, Property, Text, Values, check_presets, effective};
pub use quality::{Class, Quality, Tier};
pub use runtime::Runtime;
pub use timeline::{Key, Loop, Timeline, local, sample};
pub use uses::Uses;
pub use validate::Issue;
pub use value::{AudioChannel, Axis, Band, Binding, Level, Param, Value, Vec2};
pub use wake::Wake;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use wprs_core::Fit;

pub const SCHEMA: &str = "wallpaper-rs.scene/1";
pub const MAX_SIZE: u32 = 16_384;
pub const MAX_LAYERS: usize = 256;
pub const MAX_DEPTH: usize = 8;
pub const MAX_EFFECTS: usize = 16;
pub const MAX_SYSTEMS: usize = 8;
pub const MAX_PARTICLES: u32 = 200_000;
pub const MAX_TIMELINES: usize = 64;
pub const MAX_KEYS: usize = 256;
pub const MAX_SCRIPTS: usize = 16;
pub const MAX_ASSETS: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("invalid scene: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid scene: {0}")]
    Invalid(Issue),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "value::ColorRepr", into = "[f32; 4]")]
pub struct Color(pub [f32; 4]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendSpace {
    #[default]
    Linear,
    Srgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub size: [u32; 2],
    #[serde(default)]
    pub fit: Fit,
    #[serde(default)]
    pub background: Color,
    #[serde(default)]
    pub blend_space: BlendSpace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Clock {
    pub max_fps: u32,
    pub poster_time: f32,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub linear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<[u32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub schema: String,
    pub canvas: Canvas,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, Asset>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub particles: BTreeMap<String, System>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timelines: Vec<Timeline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<Script>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Json>,
}

impl Default for Color {
    fn default() -> Self {
        Color([0.0, 0.0, 0.0, 1.0])
    }
}

impl Default for Clock {
    fn default() -> Self {
        Clock { max_fps: 60, poster_time: 0.0, seed: 0 }
    }
}

impl Scene {
    pub fn new(size: [u32; 2]) -> Scene {
        let canvas =
            Canvas { size, fit: Fit::Fill, background: Color::default(), blend_space: BlendSpace::Linear };
        let (assets, layers, particles, timelines, scripts, ext) = Default::default();
        Scene {
            schema: SCHEMA.into(),
            canvas,
            clock: Clock::default(),
            assets,
            layers,
            particles,
            timelines,
            scripts,
            ext,
        }
    }

    pub fn from_json(text: &str) -> Result<Scene, SceneError> {
        Ok(serde_json::from_str(text)?)
    }

    pub fn load(text: &str, props: &BTreeMap<String, Property>) -> Result<Scene, SceneError> {
        let scene = Scene::from_json(text)?;
        match scene.validate(props).into_iter().next() {
            Some(issue) => Err(SceneError::Invalid(issue)),
            None => Ok(scene),
        }
    }

    pub fn to_json(&self) -> Result<String, SceneError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        let mut stack: Vec<&Layer> = self.layers.iter().rev().collect();
        std::iter::from_fn(move || {
            let layer = stack.pop()?;
            if let Kind::Group { layers, .. } = &layer.kind {
                stack.extend(layers.iter().rev());
            }
            Some(layer)
        })
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers().find(|l| l.id == id)
    }

    pub fn paths(&self) -> BTreeSet<&str> {
        let assets = self.assets.values().map(|a| a.path.as_str());
        assets.chain(self.scripts.iter().map(|s| s.path.as_str())).collect()
    }
}
