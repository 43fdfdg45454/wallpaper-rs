use serde::{Deserialize, Serialize};
use serde_json::Value;
use wprs_scene::Loop;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyframeEdit {
    pub pointer: String,
    pub time: f64,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub ease: Option<Value>,
    #[serde(default)]
    pub timeline: Option<String>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub from: Option<f64>,
    #[serde(default)]
    pub base: Option<Value>,
    #[serde(default)]
    pub absolute: bool,
    #[serde(default)]
    pub in_handle: Option<[f64; 2]>,
    #[serde(default)]
    pub out_handle: Option<[f64; 2]>,
    #[serde(default)]
    pub straighten: bool,
}

impl KeyframeEdit {
    pub fn at(pointer: &str, time: f64) -> Self {
        KeyframeEdit {
            pointer: pointer.into(),
            time,
            value: None,
            ease: None,
            timeline: None,
            remove: false,
            from: None,
            base: None,
            absolute: false,
            in_handle: None,
            out_handle: None,
            straighten: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimelineEdit {
    pub id: String,
    pub add: bool,
    pub remove: bool,
    pub to: Option<String>,
    pub duration: Option<f64>,
    pub fps: Option<f64>,
    pub frames: Option<u32>,
    pub wrap: Option<bool>,
    #[serde(rename = "loop")]
    pub repeat: Option<Loop>,
    pub speed: Option<f64>,
    pub delay: Option<f64>,
    pub autoplay: Option<bool>,
    pub trigger: Option<String>,
}
