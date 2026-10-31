use crate::power::Power;
use crate::{Error, framing, library, settings, store};
use anyhow::anyhow;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wprs_core::Fit;
use wprs_core::edit::{self, Depth, Edit, Parallax, Source};
use wprs_depth::{Device, MODEL, Map, Places, adjust, beside, read, shrink};

const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));
pub(crate) const EDITS: &str = "edits";
const EDGE: u32 = 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Model,
    Install,
    Forget,
    Make { uid: String, library: PathBuf },
    Import { uid: String, fd: i32 },
    Item { uid: String },
    Parallax { uid: String, on: Option<bool>, strength: Option<f32>, focus: Option<f32> },
    Fit { uids: Vec<String>, fit: Option<Fit> },
    Frame { uid: String, pan: [f32; 2], zoom: f32, screen: [u32; 2], done: bool },
    Unframe { uid: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Raise {
    pub(crate) parallax: Parallax,
    pub(crate) map: Option<(PathBuf, SystemTime)>,
    depth: Depth,
}

fn places() -> Result<Places, Error> {
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
    answer.unwrap_or_else(|e| json!({ "error": e.to_string() })).to_string()
}

fn answer(request: Request) -> Result<Value, Error> {
    let places = places()?;
    let part = places.file(&MODEL).with_added_extension("part");
    let folder = part.parent().unwrap_or(&places.data);
    let image = |uid: &str| library::with(|lib| Ok(lib.item(uid)?.path));
    Ok(match request {
        Request::Model => json!({
            "installed": places.installed(&MODEL).is_some(),
            "allowed": settings::downloads(),
            "title": MODEL.title, "size": format!("{:.1} MB", MODEL.bytes as f64 / 1e6),
            "hosts": MODEL.hosts(), "license": MODEL.license, "license_url": MODEL.license_url,
            "urls": MODEL.urls, "bytes": MODEL.bytes, "part": part, "agent": AGENT,
        }),
        Request::Install => {
            if !settings::downloads() {
                drop(fs::remove_file(&part));
                Err(anyhow!("downloading the model is turned off"))?;
            }
            if wprs_depth::sha256(&part).ok().as_deref() != Some(MODEL.sha256) {
                drop(fs::remove_file(&part));
                Err(anyhow!("the file from {} has another SHA-256 and was deleted", MODEL.hosts()))?;
            }
            fs::write(folder.join("NOTICE.txt"), MODEL.notice())?;
            fs::write(folder.join("LICENSE.txt"), MODEL.license_text)?;
            fs::rename(&part, places.file(&MODEL))?;
            Value::Null
        }
        Request::Forget => json!(fs::remove_dir_all(folder).is_ok()),
        Request::Make { uid, library } => {
            let image = image(&uid)?;
            let edit = edited(&image)?;
            wprs_depth::background();
            let (depth, sha) = (edit.depth.clone().unwrap_or_default(), &edit.source.sha256);
            wprs_depth::generate(&places, (&image, sha), &depth, (Some(&library), Device::Cpu), &mut |_| {})?;
            moving(&image)?
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
            json!({ "parallax": edit.moving().is_some(), "map": found(&image, &edit).is_some(),
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

pub(crate) fn shared(item: &wprs_library::Item) -> Option<Edit> {
    let edits = store::dir().ok()?.join(EDITS);
    let found = (item.kind == wprs_library::Kind::Image).then(|| edit::load(&item.path, &edits));
    found?.ok()?.map(|(_, edit)| edit)
}

pub(crate) fn adopt(item: &wprs_library::Item, synced: Option<&Value>, keep: bool) -> Result<(), Error> {
    if item.kind == wprs_library::Kind::Image
        && edit::adopt(synced, &item.path, &store::dir()?.join(EDITS), keep).map_err(Error::Setting)?
    {
        store::changed();
    }
    Ok(())
}

pub(crate) fn raised(image: &Path) -> Option<Raise> {
    let edit = loaded(image).ok()?;
    let parallax = edit.moving()?.clone();
    let map =
        found(image, &edit).and_then(|path| Some((path.clone(), fs::metadata(&path).ok()?.modified().ok()?)));
    Some(Raise { parallax, map, depth: edit.depth.unwrap_or_default() })
}

pub(crate) fn map(raise: &Raise) -> Option<Map> {
    let (path, _) = raise.map.as_ref()?;
    Some(adjust(&shrink(read(path).ok()?, EDGE), &raise.depth))
}

pub(crate) fn forget(image: &Path) -> Result<(), Error> {
    let (edit, places) = (loaded(image)?, places()?);
    let sha = &edit.source.sha256;
    let maps = [places.own(sha), places.generated(sha, &edit.depth.unwrap_or_default()), Some(beside(image))];
    maps.into_iter().flatten().for_each(|map| drop(fs::remove_file(map)));
    edit::remove(image, &store::dir()?.join(EDITS)).map(drop).map_err(Error::Setting)
}
