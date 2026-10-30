use crate::{AudioFrame, BANDS};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SPLITS: [f32; 2] = [250.0, 4_000.0];
pub const HEARD: [f32; 2] = [40.0, 16_000.0];
pub const CLOCKS: [&str; 6] = ["day", "hour", "minute", "second", "weekday", "month"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reduce {
    #[default]
    Mean,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Smooth {
    Both(f32),
    Apart { up: f32, down: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Gain {
    #[default]
    Auto,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Hearing {
    #[schemars(extend("x-unit" = "s", "x-range" = [0.001, 2]))]
    pub attack: f32,
    #[schemars(extend("x-unit" = "s", "x-range" = [0.01, 10]))]
    pub release: f32,
    pub gain: Gain,
}

impl Default for Hearing {
    fn default() -> Self {
        Hearing { attack: 0.02, release: 0.2, gain: Gain::Auto }
    }
}

impl Smooth {
    pub fn times(self) -> [f32; 2] {
        match self {
            Smooth::Both(tau) => [tau; 2],
            Smooth::Apart { up, down } => [up, down],
        }
    }
}

pub fn edge(i: usize) -> f32 {
    HEARD[0] * (HEARD[1] / HEARD[0]).powf(i as f32 / BANDS as f32)
}

pub fn center(i: usize) -> f32 {
    (edge(i) * edge(i + 1)).sqrt()
}

pub fn levels(bands: &[f32; BANDS]) -> [f32; 3] {
    let mean = |lo: f32, hi: f32| {
        let within = (0..BANDS).filter(|&b| (lo..hi).contains(&center(b)));
        let (sum, n) = within.fold((0.0, 0), |(s, n), b| (s + bands[b], n + 1));
        sum / n.max(1) as f32
    };
    [mean(0.0, SPLITS[0]), mean(SPLITS[0], SPLITS[1]), mean(SPLITS[1], f32::MAX)]
}

pub fn ease(v: f32, target: f32, dt: f32, up: f32, down: f32) -> f32 {
    let tau = if target > v { up } else { down };
    if tau <= 0.0 { target } else { v + (target - v) * (1.0 - (-dt / tau).exp()) }
}

impl Hearing {
    pub fn hear(&self, last: &AudioFrame, mut next: AudioFrame, dt: f32) -> AudioFrame {
        let source = if self.gain == Gain::Fixed { next.loud } else { next.raw };
        let [up, down] = [self.attack, self.release].map(|t| t.clamp(0.001, 10.0));
        let follow = |from: &[f32; BANDS], to: &[f32; BANDS]| {
            std::array::from_fn(|b| ease(from[b], to[b], dt, up, down))
        };
        (next.left, next.right) = (follow(&last.left, &source[0]), follow(&last.right, &source[1]));
        next.average = std::array::from_fn(|b| (next.left[b] + next.right[b]) / 2.0);
        [next.bass, next.mid, next.treble] = levels(&next.average);
        next.level = ease(last.level, next.loudness, dt, up, down);
        next
    }
}
