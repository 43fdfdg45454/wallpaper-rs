use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod keys;

pub use keys::{Key, local, sample};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Loop {
    #[default]
    Once,
    Repeat,
    Pingpong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    pub id: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    #[schemars(extend("x-unit" = "s"))]
    pub duration: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [1, 240]))]
    pub fps: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap: bool,
    #[serde(default, rename = "loop")]
    pub repeat: Loop,
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default)]
    pub delay: f32,
    #[serde(default = "yes")]
    pub autoplay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "trigger"))]
    pub trigger: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    #[schemars(extend("x-unit" = "s"))]
    pub t: f32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Handles {
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    pub before: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Playhead {
    pub(crate) t: f32,
    pub(crate) playing: bool,
    pub(crate) reversed: bool,
    pub(crate) rate: f32,
}

impl Timeline {
    pub(crate) fn origin(&self, rate: f32) -> f32 {
        if rate < 0.0 { 0.0 } else { -self.delay }
    }

    pub(crate) fn start(&self) -> Playhead {
        Playhead {
            t: self.origin(self.speed),
            playing: self.autoplay && self.trigger.as_deref().is_none_or(|t| t == "start"),
            reversed: false,
            rate: self.speed,
        }
    }

    pub(crate) fn advance(&self, head: &mut Playhead, dt: f32) {
        if head.playing {
            let back = head.reversed != (head.rate < 0.0);
            head.t += dt * head.rate.abs() * if back { -1.0 } else { 1.0 };
            let done =
                self.repeat == Loop::Once && if back { head.t <= 0.0 } else { head.t >= self.length() };
            head.playing &= !done;
        }
    }

    pub(crate) fn position(&self, head: &Playhead) -> f32 {
        let t = if head.reversed != (head.rate < 0.0) { head.t } else { head.t.max(0.0) };
        local(t, self.length(), self.repeat)
    }

    pub fn length(&self) -> f32 {
        match (self.frames, self.fps) {
            (Some(frames), Some(fps)) if self.duration == 0.0 && fps > 0.0 => frames as f32 / fps,
            _ => self.duration,
        }
    }
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}
