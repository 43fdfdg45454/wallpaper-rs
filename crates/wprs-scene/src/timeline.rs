use crate::{Scene, SceneError, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Once,
    Loop,
    PingPong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    pub target: String,
    pub keyframes: Vec<(f32, Value)>,
    #[serde(default)]
    pub easing: Easing,
    #[serde(default)]
    pub repeat: Repeat,
}

impl Easing {
    pub fn apply(self, f: f32) -> f32 {
        match self {
            Easing::Linear => f,
            Easing::EaseIn => f * f,
            Easing::EaseOut => f * (2.0 - f),
            Easing::EaseInOut => f * f * (3.0 - 2.0 * f),
            Easing::Step => f.floor(),
        }
    }
}

impl Timeline {
    pub fn value_at(&self, t: f32) -> Option<Value> {
        let (start, end) = (self.keyframes.first()?.0, self.keyframes.last()?.0);
        let span = end - start;
        let t = match self.repeat {
            Repeat::Loop if span > 0.0 => start + (t - start).rem_euclid(span),
            Repeat::PingPong if span > 0.0 => end - ((t - start).rem_euclid(2.0 * span) - span).abs(),
            _ => t,
        };
        let i = self.keyframes.partition_point(|k| k.0 <= t).clamp(1, self.keyframes.len());
        let a = &self.keyframes[i - 1];
        let b = self.keyframes.get(i).unwrap_or(a);
        let f = if b.0 > a.0 { ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        Some(a.1.lerp(&b.1, self.easing.apply(f)))
    }
}

impl Scene {
    pub fn at(&self, t: f32) -> Result<Scene, SceneError> {
        self.with(self.timelines.iter().filter_map(|tl| Some((tl.target.as_str(), tl.value_at(t)?))))
    }
}
