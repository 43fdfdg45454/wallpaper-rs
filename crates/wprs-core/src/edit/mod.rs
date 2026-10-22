mod action;
mod check;
mod curve;
mod file;
mod geometry;
mod parse;
mod set;
mod tool;

pub use action::{ACTIONS, Action, Error};
pub use curve::spline;
pub use file::{MAX_BYTES, commit, dir, load, remove, save};
pub use geometry::{Matrix, mul};
pub use parse::numbers;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub version: u32,
    #[serde(default)]
    pub source: Source,
    #[serde(default)]
    pub geometry: Geometry,
    #[serde(default)]
    pub ops: Vec<Op>,
    #[serde(default)]
    pub masks: BTreeMap<String, Mask>,
    #[serde(default)]
    pub outputs: BTreeMap<String, OutputCrop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Source {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub pixels: [u32; 2],
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Geometry {
    pub crop: Option<[f32; 4]>,
    pub rotate: u16,
    pub straighten: f32,
    pub flip: [bool; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    pub id: String,
    #[serde(default = "on")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    Exposure {
        ev: f32,
    },
    WhiteBalance {
        temperature: f32,
        #[serde(default)]
        tint: f32,
    },
    Contrast {
        amount: f32,
    },
    Saturation {
        #[serde(default)]
        saturation: f32,
        #[serde(default)]
        vibrance: f32,
    },
    Curves {
        #[serde(default)]
        rgb: Vec<[f32; 2]>,
        #[serde(default)]
        r: Vec<[f32; 2]>,
        #[serde(default)]
        g: Vec<[f32; 2]>,
        #[serde(default)]
        b: Vec<[f32; 2]>,
    },
    Blur {
        radius: f32,
    },
    Overlay {
        color: String,
        #[serde(default)]
        blend: Blend,
    },
    Vignette {
        amount: f32,
        midpoint: f32,
        roundness: f32,
        feather: f32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    Color,
}

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputCrop {
    #[serde(default)]
    pub connector: String,
    pub crop: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Span {
    pub center: [f32; 2],
    pub zoom: f32,
}

impl Default for Span {
    fn default() -> Self {
        Self { center: [0.5, 0.5], zoom: 1.0 }
    }
}

impl Default for Edit {
    fn default() -> Self {
        let (source, geometry, ops) = (Source::default(), Geometry::default(), Vec::new());
        let (masks, outputs, span) = (BTreeMap::new(), BTreeMap::new(), None);
        Self { version: VERSION, source, geometry, ops, masks, outputs, span }
    }
}

fn on() -> bool {
    true
}
