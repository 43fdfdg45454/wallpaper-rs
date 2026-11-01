use serde::{Deserialize, Serialize};
use serde_json::Value;
use wprs_scene::{Inheritable, Orient, Refresh};

pub use super::tend::{Response, Strand, TendEdit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ParticleEdit {
    Shape {
        system: String,
        emitter: usize,
        shape: Value,
    },
    Point {
        system: String,
        point: u8,
        #[serde(default)]
        at: Option<[f64; 2]>,
        #[serde(default)]
        follow: Option<bool>,
        #[serde(default)]
        angle: Option<f64>,
        #[serde(default)]
        remove: bool,
    },
    Remap {
        system: String,
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        remap: Option<Value>,
        #[serde(default)]
        remove: bool,
    },
    Cutout {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        range: Option<[f64; 3]>,
    },
    Orient {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        orient: Option<Orient>,
        #[serde(default)]
        angle: Option<f64>,
    },
    Perspective {
        system: String,
        #[serde(default)]
        focal: Option<f64>,
        #[serde(default)]
        depth: Option<[f64; 2]>,
        #[serde(default)]
        depth_speed: Option<[f64; 2]>,
        #[serde(default)]
        off: bool,
    },
    Layer {
        system: String,
        emitter: usize,
        id: String,
        #[serde(default)]
        threshold: Option<f64>,
        #[serde(default)]
        color: bool,
        #[serde(default)]
        inherit: bool,
        #[serde(default)]
        jitter: Option<f64>,
        #[serde(default)]
        refresh: Option<Refresh>,
    },
    Inherit {
        system: String,
        child: usize,
        #[serde(default)]
        inherit: Vec<Inheritable>,
        #[serde(default)]
        follow: Option<bool>,
    },
    Shader {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        shader: Option<String>,
    },
    #[serde(untagged)]
    Tend(TendEdit),
}
