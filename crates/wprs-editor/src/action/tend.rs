use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Bounce,
    Slide,
    Stop,
    Stick,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strand {
    Rope,
    Trail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TendEdit {
    Collider {
        system: String,
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        collider: Option<Value>,
        #[serde(default)]
        remove: bool,
    },
    Response {
        system: String,
        index: usize,
        response: Response,
        #[serde(default)]
        bounce: Option<f64>,
        #[serde(default)]
        friction: Option<f64>,
    },
    Force {
        system: String,
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        force: Option<Value>,
        #[serde(default)]
        remove: bool,
    },
    Boids {
        system: String,
        #[serde(default)]
        boids: Option<Value>,
        #[serde(default)]
        off: bool,
    },
    Strand {
        system: String,
        strand: Strand,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        settings: Option<Value>,
        #[serde(default)]
        off: bool,
    },
    Texture {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        texture: Option<String>,
    },
}
