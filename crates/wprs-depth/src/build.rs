use crate::brush::lift;
use crate::fill::{push_pull, simple};
use crate::guided::{Size, refine};
use crate::plan::{Plan, Region};
use crate::split::{CUT_PIXELS, Split, soft};
use crate::stack::{Band, Info, Layers, Settings, save};
use crate::windows::{Inpaint, lama};
use crate::{CACHE_BYTES, Map, Places, adjust, decode, read};
use anyhow::{Context, Result, bail, ensure};
use image::{Luma, RgbImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use wprs_core::Fill;
use wprs_core::edit::Depth;

pub const VERSION: u32 = 1;
pub const EDGE: u32 = 1920;
pub const MARGIN: f32 = 2.0;

pub(crate) fn fitted(image: RgbImage) -> RgbImage {
    match image.width().max(image.height()) > EDGE {
        true => image::DynamicImage::ImageRgb8(image).thumbnail(EDGE, EDGE).into_rgb8(),
        false => image,
    }
}

fn leveled(picture: &RgbImage, map: &Map, depth: &Depth) -> (Size, Vec<f32>) {
    let size = (picture.width() as usize, picture.height() as usize);
    let refined = refine(map, picture);
    let steps = refined.iter().map(|v| (v * 65535.0).round() as u16).collect();
    let map = Map::from_raw(size.0 as u32, size.1 as u32, steps).unwrap_or_default();
    let flat = Depth { dilate: 0, smooth: 0, ..depth.clone() };
    let mut levels: Vec<f32> =
        adjust(&map, &flat).pixels().map(|Luma([v])| f32::from(*v) / 65535.0).collect();
    lift(&mut levels, size, &depth.strokes);
    (size, levels)
}

pub fn reach(shift: f32, width: usize, split: &Split, layer: usize) -> f32 {
    let far = split.depths.last().copied().unwrap_or(0.0) - split.depths[layer];
    (shift * width as f32 * far).ceil().max(0.0) + MARGIN
}

struct Source<'a> {
    picture: &'a RgbImage,
    size: Size,
    fill: Fill,
}

fn one(
    cx: &Source,
    (region, soft): (&Region, &[f32]),
    inpaint: Option<&mut Inpaint<'_>>,
    stop: &AtomicBool,
) -> Result<(RgbaImage, Band)> {
    let Region { known, band, reached, refill, reach, far } = region;
    let has = known.contains(&true);
    let mut filled = match (has, cx.fill) {
        (false, _) => cx.picture.clone(),
        (true, Fill::Lama) => push_pull(cx.picture, known),
        (true, _) => simple(cx.picture, known),
    };
    if has && cx.fill == Fill::Lama && band.contains(&true) {
        let Some(inpaint) = inpaint else { bail!("LaMa is not loaded") };
        lama(&mut filled, band, inpaint, stop)?;
    }
    let (w, h) = cx.size;
    let mut layer = RgbaImage::new(w as u32, h as u32);
    for (k, p) in layer.pixels_mut().enumerate() {
        let rgb = if known[k] {
            cx.picture.as_raw()[k * 3..k * 3 + 3].to_vec()
        } else {
            filled.as_raw()[k * 3..k * 3 + 3].to_vec()
        };
        let feather = ((reach + CUT_PIXELS as f32 * 2.0 - reached[k]) / 2.0).clamp(0.0, 1.0);
        let alpha = if *far {
            1.0
        } else if known[k] {
            soft[k]
        } else if band[k] {
            if refill[k] { 1.0 } else { feather.max(soft[k]) }
        } else {
            0.0
        };
        let alpha = if has { alpha } else { 0.0 };
        *p = Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]);
    }
    let pixels = band.iter().filter(|b| **b).count();
    Ok((layer, Band { layer: 0, reach: *reach, pixels }))
}

pub fn layers(
    picture: &RgbImage,
    map: &Map,
    (depth, set): (&Depth, &Settings),
    mut inpaint: Option<&mut Inpaint<'_>>,
    progress: &mut dyn FnMut(u64),
    stop: &AtomicBool,
) -> Result<Layers> {
    ensure!(set.fill != Fill::Off, "the fill mode is off: there are no layers to build");
    let (size, levels) = leveled(picture, map, depth);
    progress(10);
    let plan = Plan::new(&levels, size, depth, set.count);
    let cx = Source { picture, size, fill: set.fill };
    let (mut images, mut bands) = (Vec::new(), Vec::new());
    for i in 0..plan.parts.count() {
        ensure!(!stop.load(Relaxed), "cancelled");
        let edge = soft(&plan.masks[i], size);
        let region = plan.region(i, reach(set.shift, size.0, &plan.parts, i));
        let (image, mut band) = one(&cx, (&region, &edge), inpaint.as_deref_mut(), stop)?;
        band.layer = i;
        images.push(image);
        bands.push(band);
        progress(10 + 90 * (i as u64 + 1) / plan.parts.count() as u64);
    }
    let info = Info {
        format: 1,
        size: [size.0 as u32, size.1 as u32],
        fill: set.fill.as_str().into(),
        cuts: plan.parts.cuts,
        depths: plan.parts.depths,
        bands,
    };
    Ok(Layers { info, images })
}

pub fn band(picture: &RgbImage, map: &Map, depth: &Depth, info: &Info, layer: usize) -> Result<Vec<bool>> {
    ensure!(layer < info.depths.len(), "the set has {} layers", info.depths.len());
    let (size, levels) = leveled(picture, map, depth);
    let fixed = Depth { cuts: Some(info.cuts.clone()), ..depth.clone() };
    let plan = Plan::new(&levels, size, &fixed, info.depths.len());
    ensure!(plan.parts.count() == info.depths.len(), "the layer set does not match the depth map");
    Ok(plan.region(layer, info.bands.get(layer).map_or(MARGIN, |b| b.reach)).band)
}

impl Places {
    pub fn build_layers(
        &self,
        (image, sha256): (&Path, &str),
        (depth, set): (&Depth, &Settings),
        inpaint: Option<&mut Inpaint<'_>>,
        progress: &mut dyn FnMut(u64),
        stop: &AtomicBool,
    ) -> Result<PathBuf> {
        let dest = self.layer_set(image, sha256, depth, set).context("the image has no depth map")?;
        let map = read(&self.find(image, sha256, depth).context("the image has no depth map")?)?;
        progress(2);
        let picture = fitted(decode(image)?);
        let made = layers(&picture, &map, (depth, set), inpaint, progress, stop)?;
        save(&dest, &made)?;
        self.trim(&dest, CACHE_BYTES)?;
        Ok(dest)
    }

    pub fn layer_band(
        &self,
        (image, sha256): (&Path, &str),
        depth: &Depth,
        (info, layer): (&Info, usize),
    ) -> Result<(Vec<bool>, (u32, u32))> {
        let map = read(&self.find(image, sha256, depth).context("the image has no depth map")?)?;
        let picture = fitted(decode(image)?);
        let found = band(&picture, &map, depth, info, layer)?;
        Ok((found, picture.dimensions()))
    }
}
