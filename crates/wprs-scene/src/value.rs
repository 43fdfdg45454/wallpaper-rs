use crate::signal::{Reduce, Smooth};
use crate::timeline::{Key, Loop};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod kinds;

pub(crate) use kinds::ColorRepr;
pub use kinds::{AudioChannel, Axis, Band, Level, Param, Vec2, Vec3};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "Value_for_{T}")]
pub enum Value<T> {
    Fixed(T),
    Bound(Box<Binding<T>>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
#[schemars(rename = "Binding_for_{T}")]
pub struct Binding<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "property"))]
    pub prop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<BTreeMap<String, T>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<Key<T>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "timeline"))]
    pub timeline: Option<String>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Loop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Band>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<AudioChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Axis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clamp: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth: Option<Smooth>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "layer"))]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce: Option<Reduce>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0.1, 10]))]
    pub exponent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub g: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<crate::Media>,
    #[serde(default, rename = "fn", skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, Param>>,
}

pub type Track = Box<Value<f32>>;

impl<T> Value<T> {
    pub fn binding(&self) -> Option<&Binding<T>> {
        match self {
            Value::Fixed(_) => None,
            Value::Bound(b) => Some(b),
        }
    }
}

impl<T> Binding<T> {
    pub fn sources(&self) -> usize {
        let present = [self.prop.is_some(), self.keys.is_some(), self.audio.is_some(), self.cursor.is_some()];
        let more = [self.clock.is_some(), self.script.is_some(), self.layer.is_some(), self.tracked()];
        let last = [self.media.is_some(), self.date.is_some()];
        present.into_iter().chain(more).chain(last).filter(|p| *p).count()
    }

    pub fn tracks(&self) -> [(&'static str, Option<&Value<f32>>); 6] {
        let [x, y, r, g, b, a] = [&self.x, &self.y, &self.r, &self.g, &self.b, &self.a].map(|t| t.as_deref());
        [("x", x), ("y", y), ("r", r), ("g", g), ("b", b), ("a", a)]
    }

    pub fn tracked(&self) -> bool {
        self.tracks().iter().any(|(_, t)| t.is_some())
    }

    pub(crate) fn modify(&self, x: f32) -> f32 {
        let x = self.bounds.map_or(x, |[a, b]| ((x - a) / (b - a).max(f32::EPSILON)).clamp(0.0, 1.0));
        let x = self.exponent.map_or(x, |e| x.abs().powf(e).copysign(x));
        let x = if self.invert { 1.0 - x } else { x };
        let x = x * self.scale.unwrap_or(1.0) + self.offset.unwrap_or(0.0);
        self.clamp.map_or(x, |[lo, hi]| x.clamp(lo, hi.max(lo)))
    }
}
