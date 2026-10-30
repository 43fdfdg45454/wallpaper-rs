mod adopt;
mod affine;
mod animate;
mod area;
mod asset;
mod blend;
mod bones;
mod book;
mod budget;
mod calendar;
mod canvas;
mod catalog;
mod chain;
mod clip;
mod content;
mod cost;
mod counter;
mod cube;
mod cue;
mod custom;
mod deep;
mod easing;
mod effects;
mod eval;
mod fence;
mod fields;
mod figure;
mod flash;
mod form;
mod fx;
mod gltf;
mod guard;
mod helper;
mod hitbox;
mod hoist;
mod hooks;
mod input;
mod kept;
mod kin;
mod kinds;
mod layer;
mod ledger;
mod lens;
mod light;
mod links;
mod look;
mod loops;
mod measure;
mod media;
mod memory;
mod mpeg;
mod oklab;
mod outside;
mod package;
mod particles;
mod played;
mod pointing;
mod poses;
mod probe;
mod props;
mod quality;
mod reap;
mod released;
mod resolve;
mod rig;
mod rules;
mod runtime;
mod scan;
mod schema;
mod scripted;
mod scripting;
mod shader;
mod shape;
mod sheet;
mod signal;
mod sites;
mod sound;
mod spawn;
mod spend;
mod spill;
mod stage;
mod strikes;
mod tally;
mod tier;
mod timeline;
mod tokens;
mod trials;
mod typeset;
mod uses;
mod validate;
mod value;
mod variant;
mod verdicts;
mod voice;
mod wake;
mod weigh;

pub use adopt::Files;
pub use affine::Affine;
pub use animate::Animate;
pub use area::{Bounds, covered};
pub use asset::{Asset, COLLECTIONS, Filter, MAX_FRAMES, Wrapping};
pub use blend::{BLENDS, Blend};
pub use book::{FORGIVEN, STRIKES};
pub use budget::{Budget, Heat};
pub use calendar::format as format_time;
pub use canvas::{BlendSpace, Canvas, Clock};
pub use catalog::{described, drawn, newer_params, reserved, undrawn};
pub use chain::{CHAIN_ENTRY, effect_source, fusion};
pub use clip::{
    Clip, MAX_CLIPS, MAX_SOUND_BYTES, MAX_SOUND_SECONDS, MAX_SOUND_TOTAL, MAX_STREAM_BYTES,
    MAX_STREAM_SECONDS, MAX_STREAM_TOTAL, playable,
};
pub use content::{Align, EffectUse, Gradient, Mask, MaskChannel, Shape, Spectrum, SpectrumStyle, Stroke};
pub use cube::{Lut, LutError, MAX_LUT_BYTES, MAX_LUT_SIZE, MAX_LUT_TOTAL};
pub use cue::{Course, Fired, MAX_FIRED};
pub use custom::{
    CONTRACT, EffectFiles, MAX_EFFECT_FILES, MAX_GRID, MAX_INPUTS, MAX_OPTIONS, MAX_PARAMS, MAX_PASSES,
    MAX_TARGETS, MAX_VARIANTS, MAX_WGSL, MAX_WGSL_TOTAL,
};
pub use easing::Ease;
pub use effects::{
    BLEND_WGSL, Cost, Descriptor, Entry, FRAME_WGSL, Fuse, MaskMode, Output, PRELUDE, Pace, ParamDef,
    ParamKind, PassBlend, PassDef, RELIEF_WGSL, RenderTarget, SLOTS, Solve, Stage, State, Texel, Tune, Use,
    Variant, builtin, descriptor, register,
};
pub use eval::{Content, Draw, EffectDraw, Frame};
pub use figure::{
    Attach, Coverage, Cull, Follows, MATERIALS, MAX_FIGURES, MAX_SHADOWS, Material, MeshDepth, MeshLayer,
    MeshPart, ModelLayer, Space,
};
pub use flash::{Calming, FLASH_GRID, FlashGuard, SAFE_FLASHES};
pub use form::{form, labels, order};
pub use gltf::{
    EXTENSIONS, MAX_BONES, MAX_MODEL_BYTES, MAX_MODEL_ENTRIES, MAX_MODEL_JSON, MAX_MODEL_PARTS,
    MAX_MODELS_JSON, MAX_MORPHS, MAX_NODES, MAX_TRIANGLES, ModelFile,
};
pub use guard::guarded;
pub use helper::{
    CHECK_CPU, CHECK_MEMORY, CHECK_SHADER, launch_checks_from_here, serve_check, serve_check_with,
};
pub use hitbox::{AlphaMap, Hitbox, Hitboxes};
pub use input::{
    AudioFrame, BANDS, LayerOverride, ParticleCommand, ParticleControl, PointerState, ScriptOutputs,
    SoundCommand, SoundControl, TimelineCommand, Tuning,
};
pub use layer::{Anchor, Inherit, Kind, Layer, Parallax, Solid, Transform};
pub use lens::{Camera3d, CameraKey, CameraPath, Fade, FadeTo, MAX_PATHS, PathOrder};
pub use light::{Beam, Light, Lighting, MAX_LIGHTS, Volumetric};
pub use look::{Bloom, Camera, Glare, Glide, Playback, Shake, Tonemap};
pub use loops::{MAX_INLINED, MAX_LOOP, MAX_OPS, MAX_SHADER_NODES, MAX_WORK};
pub use measure::MAX_BUILD;
pub use media::{Cover, MEDIA_TRIGGERS, Media, Reading};
pub use memory::MAX_MEMORY;
pub use outside::{CHECK_WALL, Checker, UNCHECKED, check_out_of_process, checker, helper_runs, set_checker};
pub use particles::{
    Area, Around, Bake, Between, Boids, Bow, Burst, CPU_PARTICLES, Carry, Census, Child, Cloud, Collider,
    ControlPoint, Cue, EVENT, EmitShape, Emitter, Falloff, Flock, Follow, Force, Frames, Handle, Hsv,
    Inheritable, Init, Instance, KeepBetween, KeepDistance, MAX_CHILDREN, MAX_COLLIDERS, MAX_COLLIDERS2,
    MAX_CONTROL, MAX_CURVE, MAX_DEPTH as MAX_GENERATIONS, MAX_EMIT, MAX_EMITTERS, MAX_EMITTERS2, MAX_FIELD,
    MAX_FORCES, MAX_FORCES2, MAX_HATCH, MAX_PALETTE, MAX_PARTS, MAX_POINTS, MAX_PREWARM, MAX_PREWARM2,
    MAX_REMAPS, MAX_STOPS, MAX_TRAIL, Moment, OffsetNoise, Operator, Orient, Oscillate, PARTICLE_EVENTS,
    Palette, Part, Particle, Periodic, Perspective, Placement, Play, REMAP_INPUTS, Refract, Refresh, Remap,
    RemapOp, RemapOutput, Render, Ring, Rope, STEP, SUBSTEPS, Sim, SlowNear, Source, Spread, Stack, Stretch,
    Surface, Swarm, Sweep, System, Taper, Target, Trail, Tunable, Turbulent, WARM, WARM_WORK, Wave, Wobble,
    bin, flocks, largest, mark, stacked,
};
pub use pointing::{MAX_PRESSES, MAX_TOUCHES, Press, Touch, TouchKind};
pub use props::{
    Choice, Condition, Control, Expr, ICONS, Mode, Pick, Preset, Property, Slider, Text, Values,
    check_presets, cycle, effective, exposed, icon, legacy, rename, stale, tag,
};
pub use quality::{Class, Quality};
pub use released::{newer_manifest_keys, newer_scene_keys};
pub use rig::{
    AnimClip, AnimLayer, Bend, Body, BonePhysics, BoneRule, IkChain, MAX_ANIMATION_LAYERS, MAX_BODIES,
    MAX_CHAIN, MAX_CHAINS, MAX_CLIPS_PER_MODEL, MAX_EXPRESSIONS, Reach, Rig,
};
pub use runtime::Runtime;
pub use scan::{beyond_first, imported};
pub use schema::{easings, property_schema, schema};
pub use scripted::{
    EffectControl, Extent, LayerRead, MAX_CONSOLE, MAX_LINE, MAX_SCRIPT_BYTES, MAX_SCRIPT_TEXT, MAX_SPAWNED,
    MAX_WATCHED, plain_line,
};
pub use scripting::{
    MAX_POOL_LAYERS, MAX_POOL_VERTICES, MAX_POOLS, MAX_SCRIPT_PARAMS, MAX_SCRIPTS2, MAX_TEMPLATES, ParamType,
    Pool, PoolKind, Script, ScriptApi, ScriptEvent, ScriptParam,
};
pub use shape::{MAX_ARGUMENTS, MAX_BRACKETS, MAX_OPERANDS};
pub use signal::{CLOCKS, Gain, HEARD, Hearing, Reduce, SPLITS, Smooth, center, ease, edge, levels};
pub use sites::{Site, script_inputs, script_key, sites};
pub use sound::{MAX_SOUNDS, Order, Sound, VOLUME, Voice};
pub use spend::{MAX_STATEFUL, Spend, over};
pub use strikes::{STOPPED, end_trials, forgive_strikes, keep_strikes, strike, struck, tried, trying};
pub use tier::{Gpu, Tier};
pub use timeline::{Event, Handles, Key, Loop, Timeline, local, sample};
pub use typeset::{Backing, Glyphs, Layout, MAX_FALLBACK, Outline, Shadow, TextLayer, Valign, Wrap};
pub use uses::{Uses, needs, source, tells_time, variants};
pub use validate::Issue;
pub use value::{AudioChannel, Axis, Band, Binding, Level, Param, Value, Vec2, Vec3};
pub use voice::{Playlist, Spot, Walk};
pub use wake::Wake;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const SCHEMA: &str = "wallpaper-rs.scene/1";
pub const MAX_SIZE: u32 = 16_384;
pub const MAX_LAYERS: usize = 256;
pub const MAX_DEPTH: usize = 8;
pub const MAX_EFFECTS: usize = 16;
pub const MAX_SYSTEMS: usize = 8;
pub const MAX_SYSTEMS2: usize = 32;
pub const MAX_PARTICLES: u32 = 200_000;
pub const MAX_TIMELINES: usize = 64;
pub const MAX_KEYS: usize = 256;
pub const MAX_SCRIPTS: usize = 16;
pub const MAX_ASSETS: usize = 256;
pub const MAX_EVENTS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("invalid scene: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid scene: {0}")]
    Invalid(Issue),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "value::ColorRepr", into = "[f32; 4]")]
pub struct Color(pub [f32; 4]);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    pub effects: BTreeMap<String, EffectFiles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Hearing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera3d: Option<Camera3d>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub templates: BTreeMap<String, Layer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pools: BTreeMap<String, Pool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Json>,
    #[serde(skip)]
    #[schemars(skip)]
    pub loaded: BTreeMap<String, Arc<Descriptor>>,
    #[serde(skip)]
    #[schemars(skip)]
    pub models: BTreeMap<String, Arc<ModelFile>>,
}

impl Default for Color {
    fn default() -> Self {
        Color([0.0, 0.0, 0.0, 1.0])
    }
}

impl Scene {
    pub fn new(size: [u32; 2]) -> Scene {
        let (assets, layers, particles, timelines, scripts, effects, audio, ext, loaded) = Default::default();
        let (camera3d, templates, pools, models) = Default::default();
        Scene {
            schema: SCHEMA.into(),
            canvas: Canvas::new(size),
            clock: Clock::default(),
            assets,
            layers,
            particles,
            timelines,
            scripts,
            effects,
            audio,
            camera3d,
            templates,
            pools,
            ext,
            loaded,
            models,
        }
    }

    pub fn effect(&self, id: &str) -> Option<&Descriptor> {
        self.loaded.get(id).map(|d| &**d).or_else(|| described(id).filter(|_| !self.effects.contains_key(id)))
    }

    pub fn shader(&self, id: &str) -> Option<&Descriptor> {
        self.loaded
            .get(id)
            .map(|d| &**d)
            .or_else(|| descriptor(id).filter(|_| !self.effects.contains_key(id)))
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
        let effects = self.effects.values().flat_map(|f| [f.descriptor.as_str(), f.wgsl.as_str()]);
        assets.chain(self.scripts.iter().map(|s| s.path.as_str())).chain(effects).collect()
    }
}
