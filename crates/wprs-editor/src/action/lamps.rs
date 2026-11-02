use serde::{Deserialize, Serialize};
use serde_json::Value;
use wprs_scene::Beam;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LightEdit {
    Add {
        #[serde(default)]
        light: Beam,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        into: Option<String>,
        #[serde(default)]
        at: Option<[f64; 2]>,
    },
    Set(LightSet),
    Remove {
        layer: String,
    },
    Solo {
        #[serde(default)]
        layer: Option<String>,
    },
    Mute {
        layer: String,
        #[serde(default)]
        muted: bool,
    },
    Volumetric {
        layer: String,
        #[serde(default)]
        density: Option<f64>,
        #[serde(default)]
        samples: Option<u32>,
        #[serde(default)]
        shafts: Option<f64>,
        #[serde(default)]
        off: bool,
    },
    Projection {
        layer: String,
        #[serde(default)]
        asset: Option<String>,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        off: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightSet {
    pub layer: String,
    pub light: Option<Beam>,
    pub color: Option<Value>,
    pub intensity: Option<f64>,
    pub radius: Option<f64>,
    pub z: Option<f64>,
    pub cone: Option<[f64; 2]>,
    pub length: Option<f64>,
    pub at: Option<[f64; 2]>,
    pub aim: Option<[f64; 2]>,
    pub tilt: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightingEdit {
    pub layer: String,
    pub normal: Option<String>,
    pub env: Option<String>,
    pub roughness_map: Option<String>,
    pub emissive_map: Option<String>,
    pub roughness: Option<f64>,
    pub reflection: Option<f64>,
    pub emissive: Option<f64>,
    pub strength: Option<f64>,
    pub off: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalsMake {
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default = "two")]
    pub strength: f32,
    #[serde(default = "one")]
    pub blur: f32,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Map {
    Roughness,
    Emissive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapStroke {
    pub layer: String,
    pub map: Map,
    pub radius: f32,
    #[serde(default = "super::names::half")]
    pub hardness: f32,
    #[serde(default = "super::names::whole")]
    pub value: f32,
    pub points: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AmbientEdit {
    pub ambient: Option<Value>,
    pub sky: Option<Value>,
    pub clear: bool,
}

fn one() -> f32 {
    1.0
}

fn two() -> f32 {
    2.0
}

impl Map {
    pub fn field(self) -> &'static str {
        match self {
            Map::Roughness => "roughness_map",
            Map::Emissive => "emissive_map",
        }
    }
}
