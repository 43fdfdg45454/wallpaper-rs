use crate::Map;
use image::{GrayImage, Luma, Rgba, RgbaImage};

const HUES: [[u8; 3]; 6] =
    [[66, 135, 245], [52, 199, 89], [250, 204, 21], [249, 115, 22], [239, 68, 68], [168, 85, 247]];
const BAND: [u8; 3] = [255, 0, 200];
const DIVIDER: u32 = 2;

pub fn hue(layer: usize) -> [u8; 3] {
    HUES[layer % HUES.len()]
}

pub fn histogram(map: &Map, bins: usize) -> Vec<f32> {
    let (mut counts, last) = (vec![0u32; bins.max(1)], bins.max(1) - 1);
    for v in map.as_raw() {
        counts[(usize::from(*v) * last + 32767) / 65535] += 1;
    }
    let top = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    counts.iter().map(|c| *c as f32 / top).collect()
}

pub fn outlines(map: &Map, cuts: &[f32]) -> RgbaImage {
    let (w, h) = map.dimensions();
    let layer = |x: u32, y: u32| cuts.partition_point(|c| *c <= f32::from(map.get_pixel(x, y)[0]) / 65535.0);
    RgbaImage::from_fn(w, h, |x, y| {
        let mine = layer(x, y);
        let edge = (x + 1 < w && layer(x + 1, y) != mine) || (y + 1 < h && layer(x, y + 1) != mine);
        let [r, g, b] = hue(mine);
        Rgba([r, g, b, if edge { 255 } else { 40 }])
    })
}

pub fn tint(layer: &RgbaImage, band: &[bool]) -> RgbaImage {
    let mut out = layer.clone();
    for (p, _) in out.pixels_mut().zip(band).filter(|(_, b)| **b) {
        let [r, g, b] = BAND;
        *p = Rgba([r / 2 + p[0] / 2, g / 2 + p[1] / 2, b / 2 + p[2] / 2, p[3].max(160)]);
    }
    out
}

pub fn band_mask(band: &[bool], (w, h): (u32, u32)) -> GrayImage {
    GrayImage::from_fn(w, h, |x, y| Luma([if band[(y * w + x) as usize] { 255 } else { 0 }]))
}

pub fn halves(left: &RgbaImage, right: &RgbaImage, at: f32) -> RgbaImage {
    let (w, h) = left.dimensions();
    let cut = (at.clamp(0.0, 1.0) * w as f32) as u32;
    RgbaImage::from_fn(w, h, |x, y| match x {
        x if x.abs_diff(cut) < DIVIDER => Rgba([255, 255, 255, 255]),
        x if x < cut => *left.get_pixel(x, y),
        x => *right.get_pixel(x.min(right.width() - 1), y.min(right.height() - 1)),
    })
}
