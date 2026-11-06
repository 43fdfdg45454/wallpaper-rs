use serde::{Deserialize, Serialize};
use serde_json::Value;
use wprs_scene::{Moment, Placement};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SowEdit {
    Create {
        system: String,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        parent: Option<String>,
        #[serde(default)]
        on: Option<Moment>,
    },
    Delete {
        system: String,
    },
    Settings {
        system: String,
        #[serde(default)]
        capacity: Option<u32>,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        prewarm: Option<f64>,
        #[serde(default)]
        space: Option<Placement>,
        #[serde(default)]
        rate: Option<f64>,
        #[serde(default)]
        emitter: Option<usize>,
        #[serde(default)]
        replace: bool,
        #[serde(default)]
        lock: Option<Vec<String>>,
    },
    Start {
        system: String,
        #[serde(default)]
        ranges: serde_json::Map<String, Value>,
        #[serde(default)]
        clear: Vec<String>,
    },
    Child {
        system: String,
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        child: Option<Value>,
        #[serde(default)]
        remove: bool,
    },
    Over {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        size: Option<Vec<[f64; 2]>>,
        #[serde(default)]
        color: Option<Vec<(f64, String)>>,
    },
    Render {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        stretch: Option<Value>,
        #[serde(default)]
        frames: Option<String>,
        #[serde(default)]
        fps: Option<f64>,
        #[serde(default)]
        mix: Option<bool>,
        #[serde(default)]
        brightness: Option<f64>,
        #[serde(default)]
        cycles: Option<u32>,
        #[serde(default)]
        order: Option<String>,
    },
    Emit {
        system: String,
        emitter: usize,
        #[serde(default)]
        values: serde_json::Map<String, Value>,
        #[serde(default)]
        clear: Vec<String>,
    },
    Window {
        system: String,
        index: usize,
        #[serde(default)]
        window: Option<[f64; 4]>,
    },
    Rename {
        system: String,
        to: String,
    },
}
