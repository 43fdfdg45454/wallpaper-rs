use super::{EDGE, EDITS, Raise, found, loaded, places};
use crate::{Error, store};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wprs_core::edit::{self, Depth, Edit};
use wprs_depth::{Map, adjust, beside, read, shrink};

pub(crate) fn shared(item: &wprs_library::Item) -> Option<Edit> {
    let edits = store::dir().ok()?.join(EDITS);
    let found = (item.kind == wprs_library::Kind::Image).then(|| edit::load(&item.path, &edits));
    found?.ok()?.map(|(_, edit)| edit)
}

pub(crate) fn adopt(item: &wprs_library::Item, synced: Option<&Value>, keep: bool) -> Result<(), Error> {
    if item.kind == wprs_library::Kind::Image
        && edit::adopt(synced, &item.path, &store::dir()?.join(EDITS), keep).map_err(Error::Said)?
    {
        store::changed();
    }
    Ok(())
}

pub(crate) fn stamped(image: &Path, edit: &Edit) -> Option<(PathBuf, SystemTime)> {
    found(image, edit).and_then(|path| Some((path.clone(), fs::metadata(&path).ok()?.modified().ok()?)))
}

pub(crate) fn raised(image: &Path) -> Option<Raise> {
    let edit = loaded(image).ok()?;
    let parallax = edit.moving()?.clone();
    Some(Raise { parallax, map: stamped(image, &edit), depth: edit.depth.unwrap_or_default() })
}

pub(crate) fn masking(map: Option<&Path>, edit: &Edit) -> Option<Map> {
    Some(adjust(&shrink(read(map?).ok()?, EDGE), edit.depth.as_ref().unwrap_or(&Depth::default())))
}

pub(crate) fn map(raise: &Raise) -> Option<Map> {
    let (path, _) = raise.map.as_ref()?;
    Some(adjust(&shrink(read(path).ok()?, EDGE), &raise.depth))
}

pub(crate) fn changed(image: &Path, edit: &Edit) -> bool {
    let bytes = fs::metadata(image).map_or(0, |m| m.len());
    let pixels = image::image_dimensions(image).map_or([0; 2], |(w, h)| [w, h]);
    !edit.is_empty() && edit.source.changed(bytes, pixels)
}

pub(crate) fn look(image: &Path, edit: &Edit, colour: bool) -> Result<PathBuf, Error> {
    let depth = edit.depth.clone().unwrap_or_default();
    let missing = || crate::said("android.no_depth_map", "this picture has no depth map yet", &[]);
    let map = found(image, edit).ok_or_else(missing)?;
    let key = serde_json::to_string(&(&edit.source.sha256, &depth, colour))?;
    let name = wprs_format::sha256(key.as_bytes())?;
    let out = store::dir()?.join("looks").join(format!("{}.png", &name[..24]));
    if !out.is_file() {
        let shown = wprs_depth::shrink(adjust(&shrink(read(&map)?, EDGE), &depth), 512);
        fs::create_dir_all(out.parent().unwrap_or(&out))?;
        let looks = fs::read_dir(out.parent().unwrap_or(&out))?.flatten().map(|e| e.path());
        looks.filter(|p| p.extension().is_some_and(|e| e == "png")).for_each(|p| drop(fs::remove_file(p)));
        wprs_depth::coloured(&shown, colour).save(&out)?;
    }
    Ok(out)
}

pub(crate) fn forget(image: &Path) -> Result<(), Error> {
    let (edit, places) = (loaded(image)?, places()?);
    let sha = &edit.source.sha256;
    let maps = [places.own(sha), places.generated(sha, &edit.depth.unwrap_or_default()), Some(beside(image))];
    maps.into_iter().flatten().for_each(|map| drop(fs::remove_file(map)));
    edit::remove(image, &store::dir()?.join(EDITS)).map(drop).map_err(Error::Said)
}
