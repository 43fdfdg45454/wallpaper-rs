use clap::{ArgAction, Subcommand};
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use wprs_editor::actions::ManifestEdit as M;

#[derive(Subcommand, Debug)]
pub enum Page {
    #[command(
        about = "Set how the package plays by default: frame rate, volume and muting for videos, web pages and scenes, speed, part and loop for videos"
    )]
    Playback {
        #[arg(
            long,
            value_name = "FPS",
            help = "Highest frame rate, 1 to 240, or display to follow the screen"
        )]
        max_fps: Option<String>,
        #[arg(long, help = "Volume from 0 to 1")]
        volume: Option<f64>,
        #[arg(long, action = ArgAction::Set, help = "true to start without sound")]
        muted: Option<bool>,
        #[arg(long, help = "Speed of a video, from 0.1 to 4")]
        speed: Option<f64>,
        #[arg(long, help = "Second where a video starts, 0 to 3600")]
        start: Option<f64>,
        #[arg(long, help = "Second where a video ends, after the start")]
        end: Option<f64>,
        #[arg(long = "loop", action = ArgAction::Set, help = "false plays a video once and keeps its last frame")]
        repeat: Option<bool>,
        #[arg(long, value_name = "SETTING", help = "Remove a setting")]
        clear: Vec<String>,
    },
    #[command(
        about = "Set the script run before the page, the color shown while it loads and the storage it keeps"
    )]
    Web {
        #[arg(long, value_name = "FILE", help = "A .js or .mjs file of the package")]
        preload: Option<String>,
        #[arg(long, help = "Color as #rrggbb")]
        background: Option<String>,
        #[arg(long, value_name = "KIB", help = "Local storage of the page, 0 to 5120 KiB")]
        storage_kib: Option<u32>,
        #[arg(long, value_name = "SETTING", help = "Remove a setting")]
        clear: Vec<String>,
    },
    #[command(
        about = "Replace the https origins a web page may reach, or add or remove some; network follows them in the inputs"
    )]
    Network {
        #[arg(help = "Origins such as https://api.example.org, at most 8; none removes them all")]
        origins: Vec<String>,
        #[arg(long, conflicts_with = "remove", help = "Add the origins to the ones there are")]
        add: bool,
        #[arg(long, help = "Remove the origins from the ones there are")]
        remove: bool,
    },
    #[command(
        about = "Replace the declared inputs, or add or remove some: audio, pointer, clicks, media, network"
    )]
    Inputs {
        #[arg(value_parser = ["audio", "pointer", "clicks", "media", "network", "none"], help = "Inputs; none declares none")]
        inputs: Vec<String>,
        #[arg(long, conflicts_with = "remove", help = "Add the inputs to the ones there are")]
        add: bool,
        #[arg(long, help = "Remove the inputs from the ones there are")]
        remove: bool,
    },
    #[command(about = "Copy a file into a web, picture or video package folder")]
    AddFile {
        #[arg(help = "File to copy into the package")]
        file: PathBuf,
        #[arg(long, value_name = "FOLDER", help = "Folder of the package to copy it into")]
        to: Option<String>,
    },
}

fn put(values: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    value.into_iter().for_each(|v| drop(values.insert(key.into(), v)));
}

pub(crate) fn edit(what: Page) -> M {
    let none = |list: Vec<String>| list.into_iter().filter(|o| o != "none").collect::<Vec<_>>();
    match what {
        Page::Playback { max_fps, volume, muted, speed, start, end, repeat, clear } => {
            let mut values = Map::new();
            let fps = max_fps.map(|f| f.parse::<u32>().map_or(json!(f), |n| json!(n)));
            put(&mut values, "max_fps", fps);
            put(&mut values, "volume", volume.map(|v| json!(v)));
            put(&mut values, "muted", muted.map(|v| json!(v)));
            put(&mut values, "speed", speed.map(|v| json!(v)));
            put(&mut values, "start", start.map(|v| json!(v)));
            put(&mut values, "end", end.map(|v| json!(v)));
            put(&mut values, "loop", repeat.map(|v| json!(v)));
            M::Playback { values, clear }
        }
        Page::Web { preload, background, storage_kib, clear } => {
            let mut values = Map::new();
            put(&mut values, "preload", preload.map(|v| json!(v)));
            put(&mut values, "background", background.map(|v| json!(v)));
            put(&mut values, "storage_kib", storage_kib.map(|v| json!(v)));
            M::Web { values, clear }
        }
        Page::Network { origins, add, remove } => M::Network { origins: none(origins), add, remove },
        Page::Inputs { inputs, add, remove } => M::Inputs { inputs: none(inputs), add, remove },
        Page::AddFile { file, to } => M::AddFile { file: std::fs::canonicalize(&file).unwrap_or(file), to },
    }
}
