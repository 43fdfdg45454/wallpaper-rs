use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mask {
    Linear {
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default)]
        invert: bool,
    },
    Radial {
        center: [f32; 2],
        radius: [f32; 2],
        #[serde(default)]
        feather: f32,
        #[serde(default)]
        invert: bool,
    },
    Luminance {
        range: [f32; 2],
        #[serde(default)]
        soft: f32,
        #[serde(default)]
        invert: bool,
    },
    Depth {
        range: [f32; 2],
        #[serde(default)]
        soft: f32,
        #[serde(default)]
        invert: bool,
    },
    Brush {
        strokes: Vec<Stroke>,
        #[serde(default)]
        invert: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub radius: f32,
    pub hardness: f32,
    #[serde(default)]
    pub erase: bool,
    pub points: Vec<[f32; 2]>,
}
