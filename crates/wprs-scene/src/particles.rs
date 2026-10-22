use crate::Color;
use crate::value::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub capacity: u32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub prewarm: f32,
    pub emitters: Vec<Emitter>,
    #[serde(default)]
    pub init: Init,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forces: Vec<Force>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collide: Vec<Collider>,
    #[serde(default)]
    pub render: Render,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emitter {
    pub shape: EmitShape,
    #[serde(default = "zero")]
    pub rate: Value<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<Burst>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EmitShape {
    Point([f32; 2]),
    Box([[f32; 2]; 2]),
    Circle { center: [f32; 2], radius: f32 },
    Line([[f32; 2]; 2]),
    Edge,
    Mask { texture: String, threshold: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Init {
    pub life: [f32; 2],
    pub speed: [f32; 2],
    pub angle: [f32; 2],
    pub size: [f32; 2],
    pub rotation: [f32; 2],
    pub spin: [f32; 2],
    pub color: [Color; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Force {
    Gravity([f32; 2]),
    Drag(f32),
    Turbulence { strength: f32, scale: f32 },
    Attractor { target: AttractorTarget, strength: f32 },
    Vortex { center: [f32; 2], strength: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttractorTarget {
    Point([f32; 2]),
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collider {
    pub bounds: String,
    #[serde(default)]
    pub bounce: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub size_over_life: Vec<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<Trail>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trail {
    pub length: u32,
    pub width: f32,
}

impl Default for Init {
    fn default() -> Self {
        let white = Color([1.0; 4]);
        Init {
            life: [1.0, 2.0],
            speed: [10.0, 40.0],
            angle: [0.0, 360.0],
            size: [2.0, 6.0],
            rotation: [0.0; 2],
            spin: [0.0; 2],
            color: [white, white],
        }
    }
}

impl System {
    pub fn emits(&self) -> bool {
        self.capacity > 0 && self.emitters.iter().any(|e| e.rate != Value::Fixed(0.0) || e.burst.is_some())
    }
}

fn zero() -> Value<f32> {
    Value::Fixed(0.0)
}
