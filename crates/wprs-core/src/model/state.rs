use super::{Credit, Time, Uid};
use crate::{Fit, Screen};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

model! {
    pub struct State {
        pub model: u32,
        pub version: String,
        pub adapter: String,
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
    }

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
    }

    #[derive(Copy, Eq, Hash)]
    pub enum Shown { Color, Image, Video, Scene, #[serde(other)] Unknown }

    pub struct SceneProperties {
        pub model: u32,
        pub serial: u64,
        pub output: String,
        pub package: String,
        pub title: String,
        pub properties: BTreeMap<String, Value>,
        pub values: BTreeMap<String, Value>,
        pub presets: Vec<Value>,
    }

    pub struct OutputPlaylist {
        pub uid: Uid,
        pub name: String,
        pub position: u32,
        pub count: u32,
        pub next_at: Option<Time>,
    }

    pub struct Schema {
        pub model: u32,
        pub serial: u64,
        pub options: Vec<Field>,
        pub rules: Vec<Field>,
        pub auto: Vec<Field>,
        pub preset: Vec<Field>,
        pub providers: Vec<ProviderKind>,
        pub query: Vec<QueryKey>,
        pub thumb_sizes: Vec<u32>,
        pub fits: Vec<Fit>,
    }

    pub struct Field {
        pub key: String,
        pub kind: FieldKind,
        pub default: Value,
        pub min: Option<f64>,
        pub max: Option<f64>,
        pub step: Option<f64>,
        pub values: Vec<String>,
        pub required: bool,
        pub clearable: bool,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum FieldKind {
        Bool, Int, Float, Text, Enum, Color, Interval, Times, Weekdays, List, Url,
        #[serde(other)] Unknown,
    }

    pub struct ProviderKind {
        pub kind: String,
        pub hosts: Vec<String>,
        pub options: Vec<Field>,
        pub secret: Option<SecretField>,
    }

    pub struct SecretField { pub name: String, pub required: bool }

    pub struct QueryKey { pub key: String, pub hint: String }

    pub struct Job {
        pub id: u64,
        pub kind: JobKind,
        pub target: String,
        pub state: JobState,
        pub done: u64,
        pub total: u64,
        pub started_at: Time,
        pub finished_at: Option<Time>,
        pub error: Option<JobError>,
        pub output: Option<Value>,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum JobKind { Scan, Import, Analyze, ProviderSync, RepoAdd, RepoRefresh, Install, #[serde(other)] Unknown }

    #[derive(Copy, Eq, Hash)]
    pub enum JobState { Queued, Running, Done, Failed, Cancelled, #[serde(other)] Unknown }

    pub struct JobError { pub name: String, pub message: String }

    pub struct JobsReply { pub model: u32, pub serial: u64, pub jobs: Vec<Job> }

    pub struct JobReply { pub model: u32, pub serial: u64, pub job: Job }
}
