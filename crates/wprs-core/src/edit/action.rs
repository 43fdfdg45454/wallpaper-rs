use crate::message::Message;
use serde::Deserialize;

pub const ACTIONS: [&str; 16] = [
    "set",
    "crop",
    "curve",
    "blur",
    "vignette",
    "overlay",
    "mask",
    "order",
    "span",
    "reset",
    "depth",
    "parallax",
    "fit",
    "depth_brush",
    "depth_cuts",
    "fill_mask",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Usage(Message),
    #[error("{0}")]
    Invalid(Message),
}

impl Error {
    pub fn message(&self) -> &Message {
        match self {
            Error::Usage(m) | Error::Invalid(m) => m,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Set {
        changes: Vec<String>,
    },
    Crop {
        rect: Option<String>,
        aspect: Option<String>,
        rotate: Option<u16>,
        straighten: Option<f32>,
        flip: Option<String>,
        output: Option<String>,
    },
    Curve {
        #[serde(default)]
        points: Vec<String>,
        channel: Option<String>,
        id: Option<String>,
    },
    Blur {
        radius: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    },
    Vignette {
        amount: Option<f32>,
        midpoint: Option<f32>,
        roundness: Option<f32>,
        feather: Option<f32>,
        id: Option<String>,
    },
    Overlay {
        color: Option<String>,
        blend: Option<String>,
        opacity: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    },
    Mask {
        name: String,
        spec: String,
        #[serde(default)]
        invert: Option<bool>,
    },
    Order {
        ids: Vec<String>,
    },
    Span {
        zoom: Option<f32>,
        center: Option<String>,
    },
    Reset {
        #[serde(default)]
        ids: Vec<String>,
    },
    Depth {
        near: Option<f32>,
        far: Option<f32>,
        gamma: Option<f32>,
        smooth: Option<u32>,
        dilate: Option<u32>,
        input: Option<u32>,
    },
    Parallax {
        source: Option<String>,
        strength: Option<f32>,
        focus: Option<f32>,
        steps: Option<u32>,
        smoothing: Option<u32>,
        overscan: Option<f32>,
        #[serde(default)]
        off: bool,
    },
    Fit {
        mode: String,
    },
    DepthBrush {
        points: Option<String>,
        by: Option<f32>,
        #[serde(default)]
        raise: bool,
        #[serde(default)]
        lower: bool,
        radius: Option<f32>,
        falloff: Option<f32>,
        #[serde(default)]
        clear: bool,
    },
    DepthCuts {
        cuts: Option<String>,
        #[serde(default)]
        auto: bool,
    },
    FillMask {
        points: Option<String>,
        #[serde(default)]
        refill: bool,
        #[serde(default)]
        protect: bool,
        radius: Option<f32>,
        falloff: Option<f32>,
        #[serde(default)]
        clear: bool,
    },
}
