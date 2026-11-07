use crate::fill::{push_pull, simple};
use crate::guided::{Size, refine};
use crate::split::{CUT_PIXELS, Split, distance, indices, mask, soft, split};
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

fn fitted(image: RgbImage) -> RgbImage {
    match image.width().max(image.height()) > EDGE {
        true => image::DynamicImage::ImageRgb8(image).thumbnail(EDGE, EDGE).into_rgb8(),
        false => image,
    }
}

fn levels(refined: &[f32], size: Size, depth: &Depth) -> Vec<f32> {
    let steps = refined.iter().map(|v| (v * 65535.0).round() as u16).collect();
    let map = Map::from_raw(size.0 as u32, size.1 as u32, steps).unwrap_or_default();
    let flat = Depth { dilate: 0, smooth: 0, ..depth.clone() };
    adjust(&map, &flat).pixels().map(|Luma([v])| f32::from(*v) / 65535.0).collect()
}

fn either(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(a, b)| *a || *b).collect()
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
    (own, over, soft): (&[bool], &[bool], &[f32]),
    (far, reach): (bool, f32),
    inpaint: Option<&mut Inpaint<'_>>,
    stop: &AtomicBool,
) -> Result<(RgbaImage, Band)> {
    let near = distance(over, cx.size);
    let under: Vec<bool> = near.iter().map(|d| *d <= MARGIN).collect();
    let mut known: Vec<bool> = own.iter().zip(&under).map(|(o, u)| *o && !*u).collect();
    if far && !known.contains(&true) {
        known.clone_from_slice(own);
    }
    let has = known.contains(&true);
    let reached = distance(&known, cx.size);
    let wanted = |k: usize| !known[k] && (over[k] || under[k]) && reached[k] <= reach;
    let band: Vec<bool> = (0..known.len()).map(wanted).collect();
    let mut filled = match (has, cx.fill) {
        (false, _) => cx.picture.clone(),
        (true, Fill::Lama) => push_pull(cx.picture, &known),
        (true, _) => simple(cx.picture, &known),
    };
    if has && cx.fill == Fill::Lama && band.contains(&true) {
        let Some(inpaint) = inpaint else { bail!("LaMa is not loaded") };
        lama(&mut filled, &band, inpaint, stop)?;
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
        let alpha = if far {
            1.0
        } else if known[k] {
            soft[k]
        } else if band[k] {
            feather.max(soft[k])
        } else {
            0.0
        };
        let alpha = if has { alpha } else { 0.0 };
        *p = Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]);
    }
    let pixels = band.iter().filter(|b| **b).count();
    Ok((layer, Band { layer: 0, reach, pixels }))
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
    let size = (picture.width() as usize, picture.height() as usize);
    let levels = levels(&refine(map, picture), size, depth);
    progress(10);
    let parts = split(&levels, set.count.clamp(1, 6));
    let owner = indices(&levels, &parts);
    let masks: Vec<Vec<bool>> = (0..parts.count()).map(|i| mask(&owner, i)).collect();
    let mut over = vec![vec![false; levels.len()]; parts.count()];
    for i in (1..parts.count()).rev() {
        over[i - 1] = either(&over[i], &masks[i]);
    }
    let cx = Source { picture, size, fill: set.fill };
    let (mut images, mut bands) = (Vec::new(), Vec::new());
    for i in 0..parts.count() {
        ensure!(!stop.load(Relaxed), "cancelled");
        let edge = soft(&masks[i], size);
        let reached = reach(set.shift, size.0, &parts, i);
        let given = (&masks[i][..], &over[i][..], &edge[..]);
        let (image, mut band) = one(&cx, given, (i == 0, reached), inpaint.as_deref_mut(), stop)?;
        band.layer = i;
        images.push(image);
        bands.push(band);
        progress(10 + 90 * (i as u64 + 1) / parts.count() as u64);
    }
    let info = Info {
        format: 1,
        size: [size.0 as u32, size.1 as u32],
        fill: set.fill.as_str().into(),
        cuts: parts.cuts,
        depths: parts.depths,
        bands,
    };
    Ok(Layers { info, images })
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
}
