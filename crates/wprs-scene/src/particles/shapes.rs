use super::{Handle, Num, Target};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EmitShape {
    Point([f32; 2]),
    Box(Area),
    Circle {
        center: [f32; 2],
        radius: Num,
        #[serde(default)]
        inner: Num,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sign: Option<[i8; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Handle>,
    },
    Line([[f32; 2]; 2]),
    Edge,
    Mask {
        #[schemars(extend("x-ref" = "image"))]
        texture: String,
        #[serde(default = "half")]
        threshold: Num,
    },
    Layer(Source),
    Around(Around),
    Between(Between),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "AreaRepr", into = "AreaRepr")]
pub struct Area {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub inner: Option<[f32; 2]>,
    pub at: Option<Handle>,
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum AreaRepr {
    Corners([[f32; 2]; 2]),
    Full(Room),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Room {
    from: [f32; 2],
    to: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inner: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at: Option<Handle>,
}

impl From<AreaRepr> for Area {
    fn from(repr: AreaRepr) -> Self {
        match repr {
            AreaRepr::Corners([from, to]) => Area { from, to, inner: None, at: None },
            AreaRepr::Full(Room { from, to, inner, at }) => Area { from, to, inner, at },
        }
    }
}

impl From<Area> for AreaRepr {
    fn from(a: Area) -> Self {
        match (a.inner, a.at) {
            (None, None) => AreaRepr::Corners([a.from, a.to]),
            (inner, at) => AreaRepr::Full(Room { from: a.from, to: a.to, inner, at }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refresh {
    #[default]
    Static,
    OnChange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    #[schemars(extend("x-ref" = "layer"))]
    pub id: String,
    #[serde(default = "half")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub threshold: Num,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub color: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherit: bool,
    #[serde(default)]
    #[schemars(extend("x-unit" = "px"))]
    pub jitter: f32,
    #[serde(default)]
    pub refresh: Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sweep {
    #[default]
    Repeat,
    Mirror,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Around {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Target>,
    #[schemars(extend("x-range" = [1, 4096]))]
    pub count: u32,
    #[serde(default = "turn")]
    #[schemars(extend("x-unit" = "°"))]
    pub arc: Num,
    #[schemars(extend("x-unit" = "px"))]
    pub radius: Num,
    #[serde(default)]
    pub speed: Num,
    #[serde(default)]
    pub mode: Sweep,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub restart: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bow {
    pub amount: f32,
    #[serde(default)]
    pub side: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Taper {
    #[schemars(extend("x-range" = [0, 1]))]
    pub position: f32,
    pub speed: f32,
    #[schemars(extend("x-range" = [0, 1]))]
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Between {
    pub from: Target,
    pub to: Target,
    #[schemars(extend("x-range" = [1, 4096]))]
    pub count: u32,
    #[serde(default = "whole")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub span: Num,
    #[serde(default)]
    pub mode: Sweep,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc: Option<Bow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taper: Option<Taper>,
}

impl Default for Taper {
    fn default() -> Self {
        Taper { position: 0.5, speed: 0.0, size: 0.5 }
    }
}

fn half() -> Num {
    Num::from(0.5)
}

fn whole() -> Num {
    Num::from(1.0)
}

fn turn() -> Num {
    Num::from(360.0)
}
