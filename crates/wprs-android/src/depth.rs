use crate::{Error, library, settings, store};
use anyhow::anyhow;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wprs_core::edit::{self, Depth, Edit, Motion, Parallax, Source};
use wprs_depth::{MODEL, Map, Places, adjust, beside, read, shrink};

const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));
const EDITS: &str = "edits";
const EDGE: u32 = 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Model,
    Install,
    Forget,
    Item { uid: String },
    Parallax { uid: String, on: bool },
    Make { uid: String, library: PathBuf },
    Import { uid: String, fd: i32 },
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

fn loaded(image: &Path) -> Result<Edit, Error> {
    let found = edit::load(image, &store::dir()?.join(EDITS)).map_err(Error::Setting)?;
    Ok(found.map(|(_, edit)| edit).unwrap_or_default())
}

fn edited(image: &Path) -> Result<Edit, Error> {
    let mut edit = loaded(image)?;
    if edit.source.sha256.is_empty() {
        edit.source = Source::read(image, [0; 2])?;
    }
    Ok(edit)
}

fn found(image: &Path, edit: &Edit) -> Option<PathBuf> {
    places().ok()?.find(image, &edit.source.sha256, edit.depth.as_ref().unwrap_or(&Depth::default()))
}

fn save(image: &Path, mut edit: Edit, on: bool) -> Result<Value, Error> {
    let tilt = || Parallax { source: Motion::Accelerometer, ..Parallax::default() };
    edit.parallax.get_or_insert_with(tilt).enabled = on;
    edit::commit(&edit, image, &store::dir()?.join(EDITS)).map_err(Error::Setting)?;
    store::changed();
    Ok(Value::Null)
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
        Request::Item { uid } => {
            let image = image(&uid)?;
            let edit = loaded(&image)?;
            json!({ "parallax": edit.moving().is_some(), "map": found(&image, &edit).is_some() })
        }
        Request::Parallax { uid, on } => {
            let image = image(&uid)?;
            save(&image, edited(&image)?, on)?
        }
        Request::Make { uid, library } => {
            let image = image(&uid)?;
            let edit = edited(&image)?;
            wprs_depth::background();
            let (depth, sha) = (edit.depth.clone().unwrap_or_default(), &edit.source.sha256);
            wprs_depth::generate(&places, (&image, sha), &depth, Some(&library), &mut |_| {})?;
            save(&image, edit, true)?
        }
        Request::Import { uid, fd } => {
            let image = image(&uid)?;
            let mut source = File::from(unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?);
            let mut copy = tempfile::NamedTempFile::new_in(store::dir()?)?;
            std::io::copy(&mut source, &mut copy)?;
            let edit = edited(&image)?;
            places.import(copy.path(), &image, &edit.source.sha256)?;
            save(&image, edit, true)?
        }
    })
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
