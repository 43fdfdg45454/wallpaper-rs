use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptOutputs {
    pub values: BTreeMap<String, Json>,
    pub layers: BTreeMap<String, LayerOverride>,
    pub timelines: Vec<(String, TimelineCommand)>,
    pub particles: BTreeMap<String, ParticleControl>,
    pub sounds: BTreeMap<String, SoundControl>,
    pub videos: BTreeMap<String, VideoControl>,
    pub effects: BTreeMap<String, Vec<crate::EffectControl>>,
    pub spawned: Vec<(String, String)>,
    pub order: Option<Vec<String>>,
    pub watch: Vec<String>,
    pub wake: Option<f64>,
    pub busy: bool,
    pub console: Vec<String>,
    pub stopped: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleControl {
    pub commands: Vec<ParticleCommand>,
    pub instance: Tuning,
    pub points: BTreeMap<u8, [f32; 2]>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundControl {
    pub commands: Vec<SoundCommand>,
    pub volume: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoControl {
    pub commands: Vec<VideoCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCommand {
    Play,
    Pause,
    Stop,
    Seek(f32),
    Rate(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundCommand {
    Play,
    Pause,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleCommand {
    Play,
    Pause,
    Stop,
    Emit(u32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub alpha: Option<f32>,
    pub size: Option<f32>,
    pub count: Option<f32>,
    pub speed: Option<f32>,
    pub life: Option<f32>,
    pub time_scale: Option<f32>,
    pub brightness: Option<f32>,
    pub color: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerOverride {
    pub position: Option<[f32; 2]>,
    pub scale: Option<[f32; 2]>,
    pub rotation: Option<f32>,
    pub opacity: Option<f32>,
    pub visible: Option<bool>,
    pub color: Option<[f32; 4]>,
    pub text: Option<String>,
    pub skew: Option<[f32; 2]>,
    pub origin: Option<[f32; 2]>,
    pub parallax: Option<[f32; 2]>,
    pub size: Option<crate::Extent>,
    pub font: Option<String>,
    pub align: Option<crate::Align>,
}

pub(crate) static UNSET: LayerOverride = LayerOverride {
    position: None,
    scale: None,
    rotation: None,
    opacity: None,
    visible: None,
    color: None,
    text: None,
    skew: None,
    origin: None,
    parallax: None,
    size: None,
    font: None,
    align: None,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineCommand {
    Play,
    Pause,
    Seek(f32),
    Reverse,
    Stop,
    Speed(f32),
}

impl ScriptOutputs {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
            && self.layers.is_empty()
            && self.timelines.is_empty()
            && self.particles.is_empty()
            && self.sounds.is_empty()
            && self.videos.is_empty()
            && self.effects.is_empty()
            && self.spawned.is_empty()
            && self.order.is_none()
    }
}
