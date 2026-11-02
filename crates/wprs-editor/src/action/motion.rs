use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AnimEdit {
    Layer(PlayEdit),
    Root { layer: String, on: bool },
    Preset { layer: String, bone: String, preset: String },
    Morph { layer: String, shape: String, weight: Option<f64> },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlayEdit {
    pub layer: String,
    pub index: Option<usize>,
    pub add: Option<String>,
    pub clip: Option<String>,
    pub remove: bool,
    pub weight: Option<f64>,
    pub rate: Option<f64>,
    pub additive: Option<bool>,
    pub blend_in: Option<f64>,
    pub blend_out: Option<f64>,
    pub enabled: Option<bool>,
    pub bones: Option<Vec<String>>,
    pub to: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShadowEdit {
    pub layer: String,
    pub on: Option<bool>,
    pub resolution: Option<u32>,
    pub bias: Option<f64>,
    pub automatic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PathEdit {
    Add {
        id: String,
    },
    Key(PathKey),
    Remove {
        path: String,
    },
    Set {
        path: String,
        #[serde(default, rename = "loop")]
        closed: Option<bool>,
        #[serde(default)]
        ease: Option<String>,
        #[serde(default)]
        to: Option<String>,
    },
    Order {
        order: String,
    },
    Fade {
        #[serde(default)]
        to: Option<String>,
        #[serde(default)]
        duration: Option<f64>,
        #[serde(default)]
        clear: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PathKey {
    pub path: String,
    pub time: f64,
    pub from: Option<f64>,
    pub eye: Option<[f64; 3]>,
    pub center: Option<[f64; 3]>,
    pub up: Option<[f64; 3]>,
    pub fov: Option<f64>,
    pub from_view: bool,
    pub remove: bool,
}
