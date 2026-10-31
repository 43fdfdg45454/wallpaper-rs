use crate::power::Power;
use crate::said;
use crate::{Error, framing, library, store};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs::File;
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::SystemTime;
use wprs_core::Fit;
use wprs_core::edit::{self, Depth, Edit, Parallax, Source};
use wprs_depth::{Device, Places};

pub(crate) mod maps;
pub(crate) mod model;

pub(crate) use maps::{adopt, forget, raised, shared, stamped};

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
    let found = edit::load(image, &store::dir()?.join(EDITS)).map_err(Error::Setting)?;
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
    edit::commit(&edit, image, &store::dir()?.join(EDITS)).map_err(Error::Setting)?;
    store::changed();
    Ok(())
}

fn tune(edit: &mut Edit, on: Option<bool>, strength: Option<f32>, focus: Option<f32>) -> Result<(), Error> {
    let p = edit.parallax.get_or_insert_with(Parallax::default);
    p.enabled = on.unwrap_or(p.enabled);
    (p.strength, p.focus) = (strength.unwrap_or(p.strength), focus.unwrap_or(p.focus));
    if on == Some(true) && p.strength <= 0.0 {
        p.strength = Parallax::default().strength;
    }
    Ok(())
}

fn moving(image: &Path) -> Result<Value, Error> {
    change(image, |edit| tune(edit, Some(true), None, None)).map(|()| Value::Null)
}

pub(crate) fn request(text: &str) -> String {
    let answer = serde_json::from_str(text).map_err(Error::from).and_then(answer);
    answer.unwrap_or_else(|e| e.reply()).to_string()
}

fn answer(request: Request) -> Result<Value, Error> {
    if let Request::Cancel = request {
        return Ok(json!(!STOP.swap(true, Relaxed)));
    }
    let places = places()?;
    let image = |uid: &str| library::with(|lib| Ok(crate::pack::media(&lib.item(uid)?)?.0));
    Ok(match request {
        Request::Model => model::shown(&places),
        Request::Install => model::install(&places)?,
        Request::Forget => json!(model::forget(&places)),
        Request::Make { uid, library } => {
            model::ready(&places)?;
            let image = image(&uid)?;
            let edit = edited(&image)?;
            STOP.store(false, Relaxed);
            wprs_depth::background();
            let (depth, sha) = (edit.depth.clone().unwrap_or_default(), &edit.source.sha256);
            let runtime = (library.as_deref().filter(|l| l.is_file()), Device::Cpu);
            let made =
                wprs_depth::generate_until(&places, (&image, sha), &depth, runtime, &mut |_| {}, &STOP);
            if made.as_ref().is_err_and(|e| e.is::<wprs_depth::Stopped>()) {
                Err(said("android.depth_cancelled", "the depth map was cancelled", &[]))?;
            }
            made?;
            moving(&image)?
        }
        Request::Cancel => Value::Null,
        Request::Unmap { uid } => {
            let image = image(&uid)?;
            let edit = edited(&image)?;
            let removed = places.unmap(&image, &edit.source.sha256, &edit.depth.unwrap_or_default());
            store::changed();
            json!(removed)
        }
        Request::Levels { uid, near, far, gamma, smooth, dilate } => {
            let levels = edit::Action::Depth { near, far, gamma, smooth, dilate, input: None };
            change(&image(&uid)?, |edit| edit.act(levels).map_err(|e| Error::Setting(e.to_string())))?;
            Value::Null
        }
        Request::Import { uid, fd } => {
            let image = image(&uid)?;
            let mut source = File::from(unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?);
            let mut copy = tempfile::NamedTempFile::new_in(store::dir()?)?;
            std::io::copy(&mut source, &mut copy)?;
            places.import(copy.path(), &image, &edited(&image)?.source.sha256)?;
            moving(&image)?
        }
        Request::Item { uid } => {
            let image = image(&uid)?;
            let edit = loaded(&image)?;
            let p = edit.parallax.clone().unwrap_or_default();
            let levels = edit.depth.clone().unwrap_or_default();
            json!({ "parallax": edit.moving().is_some(), "map": found(&image, &edit).is_some(),
                "near": levels.near, "far": levels.far, "gamma": levels.gamma, "smooth": levels.smooth, "dilate": levels.dilate,
                "strength": p.strength, "focus": p.focus, "fit": edit.fit, "base": edit.geometry.crop,
                "crop": framing::draft(&uid).or_else(|| framing::own(&edit)), "paused": Power::now().stills() })
        }
        Request::Parallax { uid, on, strength, focus } => {
            change(&image(&uid)?, |edit| tune(edit, on, strength, focus))?;
            Value::Null
        }
        Request::Fit { uids, fit } => framing::fit(&uids, fit)?,
        Request::Frame { uid, pan, zoom, screen, done } => framing::frame(uid, pan, zoom, screen, done)?,
        Request::Unframe { uid } => framing::unframe(&image(&uid)?, &uid)?,
    })
}
