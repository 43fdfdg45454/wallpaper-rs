use crate::{Color, Effect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: LayerKind,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub blend: Blend,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerKind {
    Image { src: String },
    Video { src: String },
    Color { color: Color },
    Text { text: String, font: String, size: f32, color: Color },
    Particles(Emitter),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub rotation: f32,
    pub origin: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Emitter {
    pub rate: f32,
    pub lifetime: [f32; 2],
    pub speed: [f32; 2],
    pub direction: f32,
    pub spread: f32,
    pub gravity: [f32; 2],
    pub size: [f32; 2],
    pub gradient: Vec<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    pub max_particles: u32,
}

impl Layer {
    pub fn new(id: impl Into<String>, kind: LayerKind) -> Layer {
        let (name, transform, blend, effects, mask) = Default::default();
        Layer { id: id.into(), name, kind, transform, opacity: 1.0, blend, visible: true, effects, mask }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Transform { x: 0.0, y: 0.0, scale: 1.0, rotation: 0.0, origin: [0.5, 0.5] }
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Emitter {
            rate: 16.0,
            lifetime: [2.0, 4.0],
            speed: [20.0, 60.0],
            direction: -90.0,
            spread: 30.0,
            gravity: [0.0, 0.0],
            size: [2.0, 6.0],
            gradient: vec![Color([1.0; 4]), Color([1.0, 1.0, 1.0, 0.0])],
            texture: None,
            max_particles: 256,
        }
    }
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}
