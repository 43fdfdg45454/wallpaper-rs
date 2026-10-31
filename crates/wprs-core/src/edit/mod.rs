mod action;
mod check;
mod curve;
mod depth;
mod fields;
mod file;
mod geometry;
mod history;
mod masks;
mod parse;
mod set;
mod tool;

use crate::Fit;
pub use action::{ACTIONS, Action, Error};
pub use curve::spline;
pub use depth::{Depth, MAX_STRENGTH, MODEL, Motion, Parallax, STEPS};
pub use file::{MAX_BYTES, adopt, commit, dir, load, remove, save};
pub use geometry::{Matrix, mul};
pub use history::History;
pub use masks::{Mask, Stroke};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallax: Option<Parallax>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,
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
        let (masks, outputs, span, depth, parallax) = (BTreeMap::new(), BTreeMap::new(), None, None, None);
        Self { version: VERSION, source, geometry, ops, masks, outputs, span, depth, parallax, fit: None }
    }
}

fn on() -> bool {
    true
}
