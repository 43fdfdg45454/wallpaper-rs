use super::{Credit, Time, Uid};
use crate::{Filter, Fit, Screen};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

model! {
    pub struct State {
        pub model: u32,
        pub version: String,
        pub adapter: String,
        #[serde(default)]
        pub tier: String,
        #[serde(default)]
        pub tier_measured: bool,
        pub config: PathBuf,
        pub config_error: Option<String>,
        pub library_error: Option<String>,
        pub serial: u64,
        pub paused: bool,
        pub on_battery: bool,
        pub locked: bool,
        pub volume: f64,
        pub muted: bool,
        pub profile: Option<Uid>,
        pub jobs: u32,
        pub options: BTreeMap<String, Value>,
        pub outputs: BTreeMap<String, OutputState>,
        #[serde(default)]
        pub calibrating: bool,
        #[serde(default)]
        pub compute: bool,
        #[serde(default)]
        pub adapters: Vec<GpuAdapter>,
        #[serde(default)]
        pub gpu_warning: Option<String>,
        #[serde(default)]
        pub gpu_restart: bool,
    }

    pub struct GpuAdapter { pub name: String, pub kind: String, pub backend: String, pub used: bool }

    pub struct OutputState {
        pub width: u32,
        pub height: u32,
        pub image: Option<PathBuf>,
        pub fit: Fit,
        #[serde(default)]
        pub background: String,
        pub kind: Shown,
        pub playing: bool,
        pub active: bool,
        pub covered: bool,
        pub clients: u32,
        pub error: Option<String>,
        pub item: Option<Uid>,
        pub preset: Option<Uid>,
        pub playlist: Option<OutputPlaylist>,
        pub credit: Option<Credit>,
        #[serde(default)]
        pub edited: bool,
        #[serde(default)]
        pub screen: Option<Screen>,
        #[serde(default)]
        pub mm: Option<[f64; 4]>,
        #[serde(default)]
        pub parallax: bool,
        #[serde(default)]
        pub frames: u64,
        #[serde(default)]
        pub refresh: u32,
        #[serde(default)]
        pub fps: u32,
        #[serde(default)]
        pub buffers: String,
        #[serde(default)]
        pub sound: String,
        #[serde(default)]
        pub shown_fit: Fit,
        #[serde(default)]
        pub flashes: String,
        #[serde(default)]
        pub left_out: Vec<String>,
        #[serde(default)]
        pub particles: String,
        #[serde(default)]
        pub quality: String,
        #[serde(default)]
        pub quality_reason: String,
        #[serde(default)]
        pub decode: String,
        #[serde(default)]
        pub console: Vec<String>,
        #[serde(default)]
        pub sandbox: String,
        #[serde(default)]
        pub accent: Option<String>,
        #[serde(default)]
        pub filter: Option<Filter>,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum Shown { Color, Image, Video, Scene, Web, #[serde(other)] Unknown }

    pub struct SceneProperties {
        pub model: u32,
        pub serial: u64,
        pub output: String,
        pub package: String,
        pub title: String,
        pub properties: BTreeMap<String, Value>,
        pub values: BTreeMap<String, Value>,
        pub presets: Vec<Value>,
        #[serde(default)]
        pub fields: Vec<SceneField>,
        #[serde(default)]
        pub preset_labels: Vec<SceneOption>,
        #[serde(default)]
        pub warnings: Vec<String>,
        #[serde(default)]
        pub user_presets: Vec<SceneOption>,
        #[serde(default)]
        pub shared: bool,
        #[serde(default)]
        pub kind: String,
    }

    pub struct SceneField {
        pub key: String,
        pub kind: String,
        pub label: String,
        pub help: String,
        pub value: Value,
        pub visible: bool,
        pub min: Option<f64>,
        pub max: Option<f64>,
        pub step: Option<f64>,
        pub integer: bool,
        pub unit: String,
        pub options: Vec<SceneOption>,
        pub alpha: bool,
        pub multiline: bool,
        pub max_len: Option<u32>,
        pub accept: Vec<String>,
        #[serde(default = "yes")]
        pub enabled: bool,
        #[serde(default)]
        pub group: String,
        #[serde(default)]
        pub icon: String,
        #[serde(default)]
        pub decimals: Option<u32>,
        #[serde(default)]
        pub editable: bool,
        #[serde(default)]
        pub log: bool,
        #[serde(default)]
        pub collapsed: bool,
        #[serde(default)]
        pub secret: bool,
        #[serde(default)]
        pub filled: bool,
    }

    pub struct SceneOption {
        pub value: Value,
        pub label: String,
        #[serde(default)]
        pub icon: String,
    }

    pub struct OutputPlaylist {
        pub uid: Uid,
        pub name: String,
        pub position: u32,
        pub count: u32,
        pub next_at: Option<Time>,
    }
}

fn yes() -> bool {
    true
}
