mod link;
mod policy;
mod spawn;

pub use link::Link;
pub use policy::{csp, filter, languages, mime};
pub use spawn::{Isolation, isolated, spawn};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::PathBuf;

pub const SCHEME: &str = "wpak";
pub const USER_SCHEME: &str = "wpak-user";
pub const BANDS: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub fps: u32,
    pub quality: u8,
    pub locale: String,
    pub volume: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowed {
    pub audio: bool,
    pub pointer: bool,
    pub clicks: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub prop: String,
    pub all: bool,
    pub files: Vec<(String, PathBuf)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Start {
    pub package: PathBuf,
    pub id: String,
    pub entry: String,
    pub preload: Option<String>,
    pub background: [u8; 3],
    pub network: Vec<String>,
    pub storage: Option<PathBuf>,
    pub languages: Vec<String>,
    pub mask_gpu: bool,
    pub device: Option<String>,
    pub modifiers: Vec<u64>,
    pub properties: Map<String, Value>,
    pub shape: Value,
    pub settings: Settings,
    pub allowed: Allowed,
    pub folders: Vec<Folder>,
    pub files: Vec<(String, PathBuf)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToWeb {
    Start(Box<Start>),
    Open { view: u32, size: (u32, u32), scale: f64, properties: Map<String, Value> },
    Close(u32),
    Resize { view: u32, size: (u32, u32), scale: f64 },
    Visible { view: u32, visible: bool },
    Fps { view: u32, fps: u32 },
    Settings(Settings),
    Properties { view: u32, values: Map<String, Value> },
    Allowed(Allowed),
    Audio { left: Vec<f32>, right: Vec<f32> },
    Pointer { view: u32, at: Option<(f64, f64)> },
    Press { view: u32, at: (f64, f64), button: u32 },
    Folder(Folder),
    Files(Vec<(String, PathBuf)>),
    Action(String),
    Released { view: u32, buffer: u32 },
    Freed { view: u32, slot: u32 },
    Copy,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FromWeb {
    Ready { version: String, sandboxed: bool },
    Unavailable(String),
    Failed(String),
    Buffer { view: u32, id: u32, size: (u32, u32), fourcc: u32, modifier: u64, offset: u32, stride: u32 },
    Forget { view: u32 },
    Frame { view: u32, buffer: u32 },
    Pixels { view: u32, slot: u32, size: (u32, u32), stride: u32, fresh: bool },
    Wants { audio: bool },
    Loaded { view: u32 },
    Crashed { view: u32 },
    Said(String),
}

pub fn changed(old: &Map<String, Value>, new: &Map<String, Value>) -> Map<String, Value> {
    let gone = old.keys().filter(|k| !new.contains_key(*k)).map(|k| (k.clone(), Value::Null));
    let fresh = new.iter().filter(|(k, v)| old.get(*k) != Some(v)).map(|(k, v)| (k.clone(), v.clone()));
    fresh.chain(gone).collect()
}

pub fn token(prop: &str, salt: &[u8; 32], path: &std::path::Path) -> String {
    let hash = blake3::keyed_hash(salt, path.as_os_str().as_encoded_bytes()).to_hex();
    format!("{USER_SCHEME}://{prop}/{}", wprs_format::page::token_name(path, &hash))
}
