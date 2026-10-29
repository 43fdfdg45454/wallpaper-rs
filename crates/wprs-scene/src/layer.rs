use crate::content::{EffectUse, Gradient, Mask, Shape, Spectrum, Stroke, mask, shown};
use crate::figure::{Attach, MeshLayer, ModelLayer, Space};
use crate::light::{Light, Lighting};
use crate::look::Playback;
use crate::particles::{Instance, Render};
use crate::typeset::TextLayer;
use crate::value::{Value, Vec2};
use crate::{Blend, Color, Sound};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Layer {
    pub id: String,
    #[serde(default, skip_serializing_if = "Transform::is_default")]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub opacity: Value<f32>,
    #[serde(default = "shown")]
    pub visible: Value<bool>,
    #[serde(default)]
    pub blend: Blend,
    #[serde(default, deserialize_with = "mask", skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectUse>,
    #[serde(default, skip_serializing_if = "Parallax::is_zero")]
    #[schemars(extend("x-range" = [-1, 1], "default" = 0))]
    pub parallax: Parallax,
    #[serde(default, skip_serializing_if = "Solid::is_off")]
    #[schemars(extend("default" = false))]
    pub solid: Solid,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cache: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 16]))]
    pub brightness: Option<Value<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "layer"))]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherit: Option<Vec<Inherit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<Attach>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub depth_test: bool,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Solid {
    Flag(bool),
    Hit {
        #[schemars(extend("x-range" = [0, 1]))]
        alpha: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Group {
        layers: Vec<Layer>,
        #[serde(default)]
        clip: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        space: Option<Space>,
    },
    Image {
        #[schemars(extend("x-ref" = "image"))]
        image: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        playback: Option<Playback>,
    },
    Video {
        #[schemars(extend("x-ref" = "video"))]
        video: Value<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        playback: Option<Playback>,
    },
    Text(Box<TextLayer>),
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
        #[schemars(extend("x-ref" = "system"))]
        system: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance: Option<Box<Instance>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        render: Option<Box<Render>>,
    },
    Spectrum(Spectrum),
    Sound(Sound),
    Solid {
        color: Value<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<[f32; 2]>,
    },
    Light(Box<Light>),
    Adjustment {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-unit" = "px"))]
        size: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        redraw: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        rgb: bool,
    },
    Mesh(Box<MeshLayer>),
    Model(Box<ModelLayer>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Transform {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub position: Option<Value<Vec2>>,
    pub scale: Value<Vec2>,
    #[schemars(extend("x-unit" = "°", "x-range" = [-360, 360]))]
    pub rotation: Value<f32>,
    pub skew: Value<Vec2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°", "x-range" = [-89, 89]))]
    pub tilt: Option<Value<Vec2>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub z: Option<Value<f32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Inherit {
    Transform,
    Visible,
    Opacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Depth", into = "Depth")]
pub struct Parallax(pub [f32; 2]);

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum Depth {
    Both(f32),
    Apart([f32; 2]),
}

impl From<Depth> for Parallax {
    fn from(depth: Depth) -> Self {
        Parallax(match depth {
            Depth::Both(v) => [v; 2],
            Depth::Apart(v) => v,
        })
    }
}

impl From<Parallax> for Depth {
    fn from(Parallax([x, y]): Parallax) -> Self {
        if x == y { Depth::Both(x) } else { Depth::Apart([x, y]) }
    }
}

impl Default for Solid {
    fn default() -> Self {
        Solid::Flag(false)
    }
}

impl Solid {
    pub fn is_off(&self) -> bool {
        *self == Solid::Flag(false)
    }
}

impl Parallax {
    pub fn is_zero(&self) -> bool {
        self.0 == [0.0; 2]
    }

    pub fn apart(&self) -> bool {
        self.0[0] != self.0[1]
    }
}

impl Default for Transform {
    fn default() -> Self {
        let (unit, zero) = (Value::Fixed(Vec2([1.0; 2])), Value::Fixed(Vec2([0.0; 2])));
        let (position, origin, tilt, z) = Default::default();
        Transform { position, scale: unit, rotation: Value::Fixed(0.0), skew: zero, origin, tilt, z }
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

impl Layer {
    pub fn new(id: impl Into<String>, kind: Kind) -> Layer {
        let (transform, anchor, blend, mask, effects, parallax) = Default::default();
        let (brightness, parent, inherit, lighting, attach) = Default::default();
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
            parallax,
            solid: Solid::default(),
            cache: false,
            brightness,
            parent,
            inherit,
            lighting,
            attach,
            depth_test: false,
            kind,
        }
    }

    pub fn children(&self) -> &[Layer] {
        match &self.kind {
            Kind::Group { layers, .. } => layers,
            _ => &[],
        }
    }

    pub fn drawn(&self) -> bool {
        !matches!(self.kind, Kind::Sound(_) | Kind::Light(_))
    }

    pub fn flat(&self) -> bool {
        self.drawn() && !matches!(self.kind, Kind::Mesh(_) | Kind::Model(_))
    }
}

fn one() -> Value<f32> {
    Value::Fixed(1.0)
}
