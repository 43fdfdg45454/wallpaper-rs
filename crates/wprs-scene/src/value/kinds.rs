use crate::{Color, Issue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
