use crate::depthwork::sha;
use anyhow::{Context, Result, bail, ensure};
use image::{DynamicImage, RgbaImage};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use wprs_core::Fill;
use wprs_core::edit::{Depth, Edit};
use wprs_depth::{Device, Places, coloured, histogram, load, outlines, settings, split, tint};

const OVERLAY_EDGE: u32 = 1024;
const OUTLINE_EDGE: u32 = 512;
const SURVEY_EDGE: u32 = 256;
const BINS: usize = 64;

pub struct Target {
    pub image: PathBuf,
    pub edit: Edit,
    pub sha: String,
}

impl Target {
    pub fn new(image: &Path, edit: &str) -> Result<Target> {
        let edit = Edit::parse(edit).map_err(|e| anyhow::anyhow!(e.local()))?;
        let sha = sha(image, &edit)?;
        Ok(Target { image: image.to_owned(), edit, sha })
    }

    pub fn depth(&self) -> Depth {
        self.edit.depth.clone().unwrap_or_default()
    }

    fn map(&self, places: &Places, edge: u32) -> Result<wprs_depth::Map> {
        let found = places.map(&self.image, &self.sha, &self.depth(), edge)?;
        found.context("the picture has no depth map yet")
    }
}

pub fn overlay(places: &Places, target: &Target, colour: bool) -> Result<RgbaImage> {
    let map = target.map(places, OVERLAY_EDGE)?;
    Ok(DynamicImage::ImageRgb8(coloured(&map, colour)).into_rgba8())
}

pub fn outline(places: &Places, target: &Target, cuts: &[f32]) -> Result<RgbaImage> {
    Ok(outlines(&target.map(places, OUTLINE_EDGE)?, cuts))
}

pub fn survey(places: &Places, target: &Target) -> Result<String> {
    let (map, depth) = (target.map(places, SURVEY_EDGE)?, target.depth());
    let set = settings(places, &target.edit, None);
    let built = |fill| {
        let set = wprs_depth::Settings { fill, ..set };
        places.layer_set(&target.image, &target.sha, &depth, &set).and_then(|p| load(&p).ok()?)
    };
    let current = built(set.fill);
    let levels: Vec<f32> = map.pixels().map(|p| f32::from(p[0]) / 65535.0).collect();
    let cuts = depth.cuts.clone();
    let cuts = cuts.or_else(|| Some(current.as_ref()?.info.cuts.clone()));
    let cuts = cuts.unwrap_or_else(|| split(&levels, set.count.clamp(1, 6)).cuts);
    let bands = current.as_ref().map(|l| l.info.bands.iter().map(|b| b.pixels).collect::<Vec<_>>());
    Ok(json!({
        "histogram": histogram(&map, BINS), "cuts": cuts, "manual": depth.cuts.is_some(),
        "fill": set.fill.as_str(), "built": current.is_some(), "bands": bands.unwrap_or_default(),
        "sets": {"lama": built(Fill::Lama).is_some(), "simple": built(Fill::Simple).is_some()},
        "lama": places.fill(Fill::Lama) == Fill::Lama,
        "strokes": depth.strokes.len(), "marks": depth.fills.len()
    })
    .to_string())
}

pub fn build(
    places: &Places,
    target: &Target,
    fill: Option<Fill>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<Option<PathBuf>> {
    let set = settings(places, &target.edit, fill);
    if set.fill == Fill::Off {
        return Ok(None);
    }
    if let Some(wanted) = fill.filter(|f| *f != set.fill) {
        bail!("{} is not installed", if wanted == Fill::Lama { "LaMa or ONNX Runtime" } else { "the fill" });
    }
    let sets = (&target.depth(), &set);
    let made =
        places.ensure_layers((&target.image, &target.sha), sets, (Device::Auto, false), progress, stop);
    made.map(Some)
}

pub fn layer(
    places: &Places,
    target: &Target,
    (index, band): (usize, bool),
    fill: Option<Fill>,
) -> Result<RgbaImage> {
    let (depth, set) = (target.depth(), settings(places, &target.edit, fill));
    ensure!(set.fill != Fill::Off, "the setting parallax.fill is off, so there are no layers");
    let path = places.layer_set(&target.image, &target.sha, &depth, &set);
    let made =
        load(&path.context("the picture has no depth map")?)?.context("the layers are not built yet")?;
    let count = made.images.len();
    let image = made.images.get(index).with_context(|| format!("the picture has {count} layers"))?;
    if !band {
        return Ok(image.clone());
    }
    let (found, _) = places.layer_band((&target.image, &target.sha), &depth, (&made.info, index))?;
    Ok(tint(image, &found))
}
