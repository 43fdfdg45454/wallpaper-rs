use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

pub const BANDS: usize = 64;
pub(crate) const SETTLE: f32 = 0.15;
const FADE: f32 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioFrame {
    pub left: [f32; BANDS],
    pub right: [f32; BANDS],
    pub average: [f32; BANDS],
    pub bands: usize,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub level: f32,
    pub beat: bool,
    pub silent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PointerState {
    pub pos: Option<[f32; 2]>,
    pub smooth: [f32; 2],
    pub velocity: [f32; 2],
    pub buttons: u8,
    pub inside: bool,
    pub presence: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptOutputs {
    pub values: BTreeMap<String, Json>,
    pub layers: BTreeMap<String, LayerOverride>,
    pub timelines: Vec<(String, TimelineCommand)>,
    pub particles: BTreeMap<String, ParticleControl>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleControl {
    pub commands: Vec<ParticleCommand>,
    pub instance: Tuning,
    pub points: BTreeMap<u8, [f32; 2]>,
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
}

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

impl Default for AudioFrame {
    fn default() -> Self {
        let zero = [0.0; BANDS];
        AudioFrame {
            left: zero,
            right: zero,
            average: zero,
            bands: BANDS,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            level: 0.0,
            beat: false,
            silent: true,
        }
    }
}

impl Default for PointerState {
    fn default() -> Self {
        let (smooth, velocity) = ([0.5; 2], [0.0; 2]);
        PointerState { pos: None, smooth, velocity, buttons: 0, inside: false, presence: 0.0 }
    }
}

impl PointerState {
    pub fn moving(&self) -> bool {
        let settling =
            self.pos.is_some_and(|p| (p[0] - self.smooth[0]).abs() + (p[1] - self.smooth[1]).abs() > 1e-4);
        settling || self.velocity != [0.0; 2] || self.presence != f32::from(u8::from(self.inside))
    }

    pub fn follow(&self, pos: Option<[f32; 2]>, buttons: u8, dt: f32) -> PointerState {
        self.follow_in(pos, buttons, dt, SETTLE)
    }

    pub fn follow_in(&self, pos: Option<[f32; 2]>, buttons: u8, dt: f32, settle: f32) -> PointerState {
        let (target, dt, omega) = (pos.unwrap_or([0.5; 2]), dt.clamp(0.0, 0.1), 4.0 / settle.max(1e-3));
        let x = omega * dt;
        let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
        let (goal, fade) = (f32::from(u8::from(pos.is_some())), dt / FADE);
        let presence = match goal - self.presence {
            gap if gap.abs() <= fade => goal,
            gap => self.presence + fade.copysign(gap),
        };
        let mut next = PointerState { pos, buttons, inside: pos.is_some(), presence, ..*self };
        for (i, goal) in target.into_iter().enumerate() {
            let change = self.smooth[i] - goal;
            let pull = (self.velocity[i] + omega * change) * dt;
            next.velocity[i] = (self.velocity[i] - omega * pull) * decay;
            next.smooth[i] = goal + (change + pull) * decay;
            if (next.smooth[i] - goal).abs() < 1e-4 && next.velocity[i].abs() < 1e-3 {
                (next.smooth[i], next.velocity[i]) = (goal, 0.0);
            }
        }
        next
    }
}

impl ScriptOutputs {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
            && self.layers.is_empty()
            && self.timelines.is_empty()
            && self.particles.is_empty()
    }
}
