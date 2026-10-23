use crate::Color;
use crate::Sound;
use crate::content::{Align, EffectUse, Gradient, Mask, Shape, Spectrum, Stroke, mask, shown, white};
use crate::value::{Value, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    #[serde(default, skip_serializing_if = "Transform::is_default")]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    #[serde(default = "one")]
    pub opacity: Value<f32>,
    #[serde(default = "shown")]
    pub visible: Value<bool>,
    #[serde(default)]
    pub blend: Blend,
    #[serde(default, deserialize_with = "mask", skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectUse>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub parallax: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solid: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cache: bool,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Group {
        layers: Vec<Layer>,
        #[serde(default)]
        clip: bool,
    },
    Image {
        image: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
    },
    Video {
        video: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
    },
    Text {
        text: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default = "text_size")]
        size: f32,
        #[serde(default = "white")]
        color: Value<Color>,
        #[serde(default)]
        align: Align,
    },
    Shape {
        shape: Shape,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gradient: Option<Gradient>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Stroke>,
    },
    Particles {
        system: String,
    },
    Spectrum(Spectrum),
    Sound(Sound),
    Solid {
        color: Value<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
    },
    Adjustment {},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Value<Vec2>>,
    pub scale: Value<Vec2>,
    pub rotation: Value<f32>,
    pub skew: Value<Vec2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
    Lighten,
    Darken,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    ColorDodge,
    ColorBurn,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl Default for Transform {
    fn default() -> Self {
        let (unit, zero) = (Value::Fixed(Vec2([1.0; 2])), Value::Fixed(Vec2([0.0; 2])));
        Transform { position: None, scale: unit, rotation: Value::Fixed(0.0), skew: zero, origin: None }
    }
}

impl Transform {
    fn is_default(&self) -> bool {
        *self == Transform::default()
    }
}

impl Anchor {
    pub fn fraction(self) -> [f32; 2] {
        let i = self as usize;
        [(i % 3) as f32 / 2.0, (i / 3) as f32 / 2.0]
    }
}

impl Blend {
    pub fn fixed(self) -> bool {
        matches!(self, Blend::Normal | Blend::Add | Blend::Multiply | Blend::Screen)
    }

    pub fn associative(self) -> bool {
        matches!(self, Blend::Normal | Blend::Add)
    }
}

impl Layer {
    pub fn new(id: impl Into<String>, kind: Kind) -> Layer {
        let (transform, anchor, blend, mask, effects) = Default::default();
        let (opacity, visible) = (one(), shown());
        Layer {
            id: id.into(),
            transform,
            anchor,
            opacity,
            visible,
            blend,
            mask,
            effects,
            parallax: 0.0,
            solid: false,
            cache: false,
            kind,
        }
    }

    pub fn children(&self) -> &[Layer] {
        match &self.kind {
            Kind::Group { layers, .. } => layers,
            _ => &[],
        }
    }
}

fn one() -> Value<f32> {
    Value::Fixed(1.0)
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn text_size() -> f32 {
    48.0
}
