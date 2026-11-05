use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use wprs_scene::MaskChannel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectsEdit {
    Add {
        layer: String,
        effect: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        at: Option<usize>,
        #[serde(default)]
        params: Map<String, Value>,
        #[serde(default)]
        optional: bool,
    },
    Set {
        layer: String,
        index: usize,
        #[serde(default)]
        params: Map<String, Value>,
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        optional: Option<bool>,
    },
    Remove {
        layer: String,
        index: usize,
    },
    Move {
        layer: String,
        index: usize,
        to: usize,
    },
    Mask {
        layer: String,
        index: usize,
        #[serde(default)]
        texture: Option<String>,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        shape: Option<Value>,
        #[serde(default)]
        channel: Option<MaskChannel>,
        #[serde(default)]
        invert: Option<bool>,
        #[serde(default)]
        feather: Option<f64>,
        #[serde(default)]
        strength: Option<f64>,
        #[serde(default)]
        off: bool,
    },
}
