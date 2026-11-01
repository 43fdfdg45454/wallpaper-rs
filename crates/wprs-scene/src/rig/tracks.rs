use crate::timeline::Key;
use crate::value::Vec2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BoneKeys {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-unit" = "°"))]
    pub rotation: Vec<Key<f32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-unit" = "px"))]
    pub position: Vec<Key<Vec2>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scale: Vec<Key<Vec2>>,
}

impl BoneKeys {
    pub fn last(&self) -> f32 {
        let ends = [self.rotation.last().map(|k| k.t), self.position.last().map(|k| k.t)];
        ends.into_iter().chain([self.scale.last().map(|k| k.t)]).flatten().fold(0.0, f32::max)
    }
}
