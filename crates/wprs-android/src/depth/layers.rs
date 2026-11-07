use super::{STOP, places};
use crate::power::Power;
use crate::{Error, said, settings};
use anyhow::Context;
use image::{GrayImage, RgbImage};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use wprs_core::Fill;
use wprs_core::edit::Edit;
use wprs_depth::{Device, Inpainter, Settings, threads};

pub(crate) fn fill(chosen: Fill, saver: bool) -> Option<Fill> {
    match chosen {
        Fill::Off => None,
        Fill::Lama if saver => Some(Fill::Simple),
        other => Some(other),
    }
}

fn settings_of(edit: &Edit) -> Option<Settings> {
    let config = settings::config().parallax;
    let fill = places().ok()?.fill(fill(config.fill, Power::now().saver)?);
    Some(Settings { fill, count: usize::from(config.layers), shift: edit.moving()?.strength })
}

pub(crate) fn shown() -> Value {
    let (chosen, saver) = (settings::config().parallax.fill, Power::now().saver);
    json!({ "chosen": chosen.as_str(), "used": fill(chosen, saver).map(Fill::as_str), "saver": saver })
}

pub(crate) fn path(image: &Path, edit: &Edit) -> Option<PathBuf> {
    let (places, set) = (places().ok()?, settings_of(edit)?);
    let (depth, sha) = (edit.depth.clone().unwrap_or_default(), &edit.source.sha256);
    let exact = places.layer_set(image, sha, &depth, &set).filter(|p| p.is_file());
    let newest = || {
        places.layers_of(sha).into_iter().max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
    };
    exact.or_else(newest)
}

pub(crate) fn build(
    image: &Path,
    edit: &Edit,
    library: Option<&Path>,
    device: Device,
) -> Result<PathBuf, Error> {
    let places = places()?;
    let off = || said("android.layers_off", "the fill is off: pictures keep the single image warp", &[]);
    let set = settings_of(edit).ok_or_else(off)?;
    let threads = (threads() / 2).max(1);
    let mut loaded =
        (set.fill == Fill::Lama).then(|| Inpainter::open(&places, library, threads, device)).transpose()?;
    let mut paint =
        |image: &RgbImage, mask: &GrayImage| loaded.as_mut().context("LaMa is not loaded")?.run(image, mask);
    let inpaint: Option<&mut wprs_depth::Inpaint<'_>> = (set.fill == Fill::Lama).then_some(&mut paint);
    let depth = edit.depth.clone().unwrap_or_default();
    let made =
        places.build_layers((image, &edit.source.sha256), (&depth, &set), inpaint, &mut |_| {}, &STOP)?;
    Ok(made)
}

pub(crate) fn best(image: &Path, library: Option<&Path>, device: Device) -> Value {
    let built = super::edited(image).and_then(|edit| build(image, &edit, library, device));
    built.inspect_err(|e| log::warn!("cannot build the depth layers: {e:?}")).ok();
    Value::Null
}
