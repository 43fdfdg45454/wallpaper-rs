use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wprs_format::OriginKind;
use wprs_scene::Blend;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerEdit {
    Transform {
        layer: String,
        #[serde(default)]
        position: Option<[f64; 2]>,
        #[serde(default)]
        rotation: Option<f64>,
        #[serde(default)]
        scale: Option<[f64; 2]>,
        #[serde(default)]
        skew: Option<[f64; 2]>,
        #[serde(default)]
        origin: Option<[f64; 2]>,
        #[serde(default)]
        tilt: Option<[f64; 2]>,
        #[serde(default)]
        z: Option<f64>,
        #[serde(default)]
        size: Option<[f64; 2]>,
        #[serde(default)]
        at: Option<f64>,
        #[serde(default)]
        replace: bool,
    },
    Blend {
        layer: String,
        blend: Blend,
    },
    Instance {
        layer: String,
        #[serde(default)]
        values: Map<String, Value>,
        #[serde(default)]
        clear: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ManifestEdit {
    Origin {
        kind: OriginKind,
        #[serde(default)]
        derived_from: Vec<Value>,
        #[serde(default)]
        provider: Option<Value>,
        #[serde(default)]
        source_url: Option<String>,
    },
    License {
        license: String,
    },
    Tags {
        tags: Vec<String>,
        #[serde(default)]
        add: bool,
        #[serde(default)]
        remove: bool,
    },
    Authors {
        authors: Vec<String>,
    },
    Display {
        #[serde(default)]
        values: Map<String, Value>,
        #[serde(default)]
        clear: Vec<String>,
    },
    Playback {
        #[serde(default)]
        values: Map<String, Value>,
        #[serde(default)]
        clear: Vec<String>,
    },
    Web {
        #[serde(default)]
        values: Map<String, Value>,
        #[serde(default)]
        clear: Vec<String>,
    },
    Network {
        origins: Vec<String>,
        #[serde(default)]
        add: bool,
        #[serde(default)]
        remove: bool,
    },
    Inputs {
        inputs: Vec<String>,
        #[serde(default)]
        add: bool,
        #[serde(default)]
        remove: bool,
    },
    AddFile {
        file: std::path::PathBuf,
        #[serde(default)]
        to: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyEdit {
    pub id: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub order: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CanvasEdit {
    Bloom {
        #[serde(default)]
        threshold: Option<f64>,
        #[serde(default)]
        strength: Option<f64>,
        #[serde(default)]
        radius: Option<f64>,
        #[serde(default)]
        tint: Option<String>,
        #[serde(default)]
        hdr: Option<Value>,
        #[serde(default)]
        off: bool,
    },
    Parallax {
        #[serde(default)]
        amount: Option<f64>,
        #[serde(default)]
        delay: Option<f64>,
        #[serde(default)]
        pointer: Option<f64>,
        #[serde(default)]
        gyro: Option<bool>,
        #[serde(default)]
        off: bool,
    },
}
