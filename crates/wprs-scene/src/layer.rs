use crate::Blend;
use crate::content::{EffectUse, Mask, mask, shown};
use crate::figure::Attach;
use crate::light::Lighting;
use crate::value::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod kind;
mod transform;

pub use kind::{Kind, Solid};
pub use transform::{Anchor, Inherit, Parallax, Transform};

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
