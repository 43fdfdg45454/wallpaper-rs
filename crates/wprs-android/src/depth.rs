use crate::{Error, store};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;
use wprs_core::Fit;
use wprs_core::edit::{self, Depth, Edit, Parallax, Source};
use wprs_depth::Places;

pub(crate) mod maps;
pub(crate) mod model;

pub(crate) use maps::{adopt, forget, raised, shared, stamped};

mod answer;

use answer::answer;

static STOP: AtomicBool = AtomicBool::new(false);
pub(crate) const EDITS: &str = "edits";
pub(crate) const EDGE: u32 = 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Model,
    Install,
    Forget,
    Make {
        uid: String,
        library: Option<PathBuf>,
        device: Option<String>,
    },
    Look {
        uid: String,
        colour: bool,
    },
    Cancel,
    Unmap {
        uid: String,
    },
    Levels {
        uid: String,
        near: Option<f32>,
        far: Option<f32>,
        gamma: Option<f32>,
        smooth: Option<u32>,
        dilate: Option<u32>,
    },
    Import {
        uid: String,
        fd: i32,
    },
    Item {
        uid: String,
    },
    Parallax {
        uid: String,
        on: Option<bool>,
        strength: Option<f32>,
        focus: Option<f32>,
    },
    Fit {
        uids: Vec<String>,
        fit: Option<Fit>,
    },
    Frame {
        uid: String,
        pan: [f32; 2],
        zoom: f32,
        screen: [u32; 2],
        done: bool,
    },
    Unframe {
        uid: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Raise {
    pub(crate) parallax: Parallax,
    pub(crate) map: Option<(PathBuf, SystemTime)>,
    pub(crate) depth: Depth,
}

pub(crate) fn places() -> Result<Places, Error> {
    let dir = store::dir()?.to_path_buf();
    Ok(Places { data: dir.clone(), cache: dir })
}

pub(crate) fn loaded(image: &Path) -> Result<Edit, Error> {
    let found = edit::load(image, &store::dir()?.join(EDITS)).map_err(Error::Said)?;
    Ok(found.map(|(_, edit)| edit).unwrap_or_default())
}

pub(crate) fn edited(image: &Path) -> Result<Edit, Error> {
    let mut edit = loaded(image)?;
    if edit.source.sha256.is_empty() {
        edit.source = Source::read(image, [0; 2])?;
    }
    Ok(edit)
}

pub(crate) fn found(image: &Path, edit: &Edit) -> Option<PathBuf> {
    places().ok()?.find(image, &edit.source.sha256, edit.depth.as_ref().unwrap_or(&Depth::default()))
}

pub(crate) fn change(image: &Path, change: impl FnOnce(&mut Edit) -> Result<(), Error>) -> Result<(), Error> {
    let mut edit = edited(image)?;
    change(&mut edit)?;
    edit::commit(&edit, image, &store::dir()?.join(EDITS)).map_err(Error::Said)?;
    store::changed();
    Ok(())
}

pub(crate) fn request(text: &str) -> String {
    let answer = serde_json::from_str(text).map_err(Error::from).and_then(answer);
    answer.unwrap_or_else(|e| e.reply()).to_string()
}
