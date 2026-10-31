use crate::pack::Pack;
use crate::props::change;
use crate::{Error, media, said, store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::os::fd::BorrowedFd;
use std::path::{Path, PathBuf};
use wprs_scene::{Control, Property};

const CHOSEN: &str = "chosen";
const TREES: &str = "trees.json";
const EDGE: u32 = 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Tree {
    tree: String,
    dir: PathBuf,
    accept: Vec<String>,
    max: u32,
    depth: u8,
}

fn trees() -> BTreeMap<String, Tree> {
    let read = store::dir().ok().and_then(|dir| fs::read(dir.join(CHOSEN).join(TREES)).ok());
    read.and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn keep(trees: &BTreeMap<String, Tree>) -> Result<(), Error> {
    fs::create_dir_all(store::dir()?.join(CHOSEN))?;
    store::write(&store::dir()?.join(CHOSEN).join(TREES), &serde_json::to_vec_pretty(trees)?)
}

fn name(pack: &Pack, key: &str) -> String {
    format!("{}/{key}", pack.id)
}

fn property<'a>(pack: &'a Pack, key: &str, folder: bool) -> Result<&'a Property, Error> {
    let found = pack.properties.get(key).filter(|p| matches!(p.control, Control::Folder { .. }) == folder);
    let found = found.filter(|p| folder || matches!(p.control, Control::File { .. }));
    found.ok_or_else(|| said("android.property_set_here", "no property %1 to set here", &[&key]))
}

fn place(pack: &Pack, key: &str) -> Result<PathBuf, Error> {
    let safe = |text: &str| {
        text.chars().map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' }).collect::<String>()
    };
    let dir = store::dir()?.join(CHOSEN).join(safe(&pack.id)).join(safe(key));
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(crate) fn kind(path: &Path) -> Option<&'static str> {
    match media::extension(path)? {
        _ if media::is_video(path) => Some("video"),
        "wpak" => None,
        _ => wprs_render::image_size(path).is_ok().then_some("image"),
    }
}

fn set(pack: &Pack, key: &str, value: Option<&Path>) -> Result<(), Error> {
    let text = value.map(|path| toml::Value::String(path.to_string_lossy().into_owned()));
    change(pack, |values| match text {
        Some(text) => drop(values.insert(key.to_owned(), text)),
        None => drop(values.remove(key)),
    })
}

pub(crate) fn file(pack: &Pack, key: &str, fd: i32, name: &str) -> Result<Value, Error> {
    let prop = property(pack, key, false)?;
    let mut source = File::from(unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?);
    let mut copy = tempfile::NamedTempFile::new_in(store::dir()?)?;
    std::io::copy(&mut source, &mut copy)?;
    let found = kind(copy.path()).filter(|kind| prop.accepts(kind));
    let found =
        found.ok_or_else(|| said("android.not_accepted", "%1 cannot be used for %2", &[&name, &key]))?;
    if found == "image" {
        wprs_render::decode_image(copy.path(), EDGE).map_err(|_| Error::Unreadable(name.into()))?;
    }
    let extension = media::extension(copy.path()).unwrap_or("bin");
    let file = place(pack, key)?.join(format!("chosen.{extension}"));
    copy.persist(&file).map_err(|e| e.error)?;
    set(pack, key, Some(&file))?;
    Ok(json!(file))
}

pub(crate) fn folder(pack: &Pack, key: &str, tree: &str) -> Result<Value, Error> {
    let prop = property(pack, key, true)?;
    let Control::Folder { max_files, recursive, .. } = prop.control else { return Ok(Value::Null) };
    let accept = ["image", "video"].into_iter().filter(|a| prop.accepts(a)).map(String::from).collect();
    let plan =
        Tree { tree: tree.to_owned(), dir: place(pack, key)?, accept, max: max_files, depth: recursive };
    let mut all = trees();
    all.insert(name(pack, key), plan.clone());
    keep(&all)?;
    Ok(json!(plan))
}

pub(crate) fn folded(pack: &Pack, key: &str) -> Result<Value, Error> {
    let plan = trees().remove(&name(pack, key));
    let dir = plan.map(|plan| plan.dir).filter(|dir| !listed(dir).is_empty());
    let dir =
        dir.ok_or_else(|| said("android.folder_nothing", "the folder has no picture or video to use", &[]))?;
    set(pack, key, Some(&dir))?;
    store::changed();
    Ok(json!(dir))
}

pub(crate) fn clear(pack: &Pack, key: &str) -> Result<Value, Error> {
    let mut all = trees();
    let tree = all.remove(&name(pack, key)).map(|plan| plan.tree);
    keep(&all)?;
    drop(fs::remove_dir_all(place(pack, key)?));
    set(pack, key, None)?;
    Ok(json!(tree))
}

pub(crate) fn mirrors() -> Value {
    json!(trees().into_iter().map(|(name, plan)| json!({ "name": name, "plan": plan })).collect::<Vec<_>>())
}

pub(crate) fn listed(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    files.retain(|file| file.is_file() && kind(file).is_some());
    files.sort();
    files
}
