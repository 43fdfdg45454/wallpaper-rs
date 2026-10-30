use super::place::half;
use super::{Around, Between, Handle, Num, Source};
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
