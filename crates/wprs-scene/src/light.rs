use crate::Color;
use crate::content::white;
use crate::value::Value;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_LIGHTS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Beam {
    #[default]
    Point,
    Spot,
    Tube,
    Directional,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Light {
    #[serde(default)]
    pub light: Beam,
    #[serde(default = "white")]
    pub color: Value<Color>,
    #[serde(default = "one")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub intensity: Value<f32>,
    #[serde(default = "reach")]
    #[schemars(extend("x-unit" = "px", "x-range" = [1, 100000]))]
    pub radius: Value<f32>,
    #[serde(default = "height")]
    #[schemars(extend("x-unit" = "px", "x-range" = [-100000, 100000]))]
    pub z: Value<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°", "x-range" = [0, 180]))]
    pub cone: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px", "x-range" = [1, 100000]))]
    pub length: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volumetric: Option<Volumetric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<Value<String>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Volumetric {
    #[schemars(extend("x-range" = [0, 1]))]
    pub density: f32,
    #[schemars(extend("x-range" = [4, 64]))]
    pub samples: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Lighting {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub normal: Option<String>,
    #[schemars(extend("x-range" = [0, 1]))]
    pub roughness: Value<f32>,
    #[schemars(extend("x-range" = [0, 1]))]
    pub reflection: Value<f32>,
    #[schemars(extend("x-range" = [0, 16]))]
    pub emissive: Value<f32>,
    #[schemars(extend("x-range" = [0, 1]))]
    pub strength: Value<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub env: Option<String>,
}

impl Default for Volumetric {
    fn default() -> Self {
        Volumetric { density: 0.2, samples: 16 }
    }
}

impl Default for Lighting {
    fn default() -> Self {
        let [roughness, reflection, emissive, strength] = [0.5, 0.0, 0.0, 1.0].map(Value::Fixed);
        Lighting { normal: None, roughness, reflection, emissive, strength, env: None }
    }
}

fn one() -> Value<f32> {
    Value::Fixed(1.0)
}

fn reach() -> Value<f32> {
    Value::Fixed(512.0)
}

fn height() -> Value<f32> {
    Value::Fixed(100.0)
}
