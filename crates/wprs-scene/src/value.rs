use crate::signal::{Reduce, Smooth};
use crate::timeline::{Key, Loop};
use crate::{Color, Issue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Band {
    Index(u32),
    Named(Level),
    Range([u32; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Bass,
    Mid,
    Treble,
    Level,
    Beat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannel {
    #[default]
    Average,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
    Inside,
    Speed,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Vec2Repr", into = "[f32; 2]")]
pub struct Vec2(pub [f32; 2]);

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum Vec2Repr {
    One(f32),
    Two([f32; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Vec3Repr", into = "[f32; 3]")]
pub struct Vec3(pub [f32; 3]);

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum Vec3Repr {
    One(f32),
    Three([f32; 3]),
}

impl From<Vec3Repr> for Vec3 {
    fn from(repr: Vec3Repr) -> Self {
        Vec3(match repr {
            Vec3Repr::One(v) => [v; 3],
            Vec3Repr::Three(v) => v,
        })
    }
}

impl From<Vec3> for [f32; 3] {
    fn from(v: Vec3) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Param {
    Bool(bool),
    Number(f32),
    Vec2([f32; 2]),
    Color(Color),
    Text(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum ColorRepr {
    Hex(String),
    Rgb([f32; 3]),
    Rgba([f32; 4]),
}

impl From<Vec2Repr> for Vec2 {
    fn from(repr: Vec2Repr) -> Self {
        Vec2(match repr {
            Vec2Repr::One(v) => [v; 2],
            Vec2Repr::Two(v) => v,
        })
    }
}

impl From<Vec2> for [f32; 2] {
    fn from(v: Vec2) -> Self {
        v.0
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        c.0
    }
}

impl TryFrom<ColorRepr> for Color {
    type Error = Issue;

    fn try_from(repr: ColorRepr) -> Result<Self, Issue> {
        match repr {
            ColorRepr::Hex(hex) => Color::parse(&hex).ok_or(Issue::Color(hex)),
            ColorRepr::Rgb([r, g, b]) => Ok(Color([r, g, b, 1.0])),
            ColorRepr::Rgba(c) => Ok(Color(c)),
        }
    }
}

impl Color {
    pub fn parse(text: &str) -> Option<Color> {
        let hex = text.strip_prefix('#')?;
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok().map(|b| f32::from(b) / 255.0);
        match hex.len() {
            6 if hex.is_ascii() => Some(Color([byte(0)?, byte(2)?, byte(4)?, 1.0])),
            8 if hex.is_ascii() => Some(Color([byte(0)?, byte(2)?, byte(4)?, byte(6)?])),
            _ => None,
        }
    }

    pub fn hex(&self) -> String {
        let [r, g, b, a] = self.0.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
    }
}

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
