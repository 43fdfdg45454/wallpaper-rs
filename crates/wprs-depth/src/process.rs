use crate::Map;
use image::imageops::{FilterType, resize};
use image::{Luma, Rgb, RgbImage};
use wprs_core::edit::Depth;

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const PATCH: f32 = 14.0;

fn scale((w, h): (u32, u32), input: u32) -> f32 {
    let (short, long) = (w.min(h).max(1) as f32, w.max(h).max(1) as f32);
    (input as f32 / short).min(2.0 * input as f32 / long)
}

pub fn input_size(image: (u32, u32), input: u32) -> (u32, u32) {
    let s = scale(image, input);
    let patches = |v: u32| ((v as f32 * s / PATCH).round().max(1.0) * PATCH) as u32;
    (patches(image.0), patches(image.1))
}

pub fn tensor(image: &RgbImage, input: u32) -> (Vec<f32>, (u32, u32)) {
    let (w, h) = input_size(image.dimensions(), input);
    let resized = resize(image, w, h, FilterType::Triangle);
    let plane = (w * h) as usize;
    let mut data = vec![0.0; 3 * plane];
    for (i, pixel) in resized.pixels().enumerate() {
        for c in 0..3 {
            data[c * plane + i] = (f32::from(pixel[c]) / 255.0 - MEAN[c]) / STD[c];
        }
    }
    (data, (w, h))
}

pub fn normalize(raw: &[f32], (w, h): (u32, u32), image: (u32, u32), input: u32) -> Map {
    let mut sorted: Vec<f32> = raw.iter().copied().filter(|v| v.is_finite()).collect();
    sorted.sort_unstable_by(f32::total_cmp);
    let at = |q: f32| sorted.get(((sorted.len().max(1) - 1) as f32 * q) as usize).copied().unwrap_or(0.0);
    let (lo, hi) = (at(0.01), at(0.99));
    let span = (hi - lo).max(1e-6);
    let level = |v: f32| (((v - lo) / span).clamp(0.0, 1.0) * 65535.0).round() as u16;
    let map = Map::from_fn(w, h, |x, y| Luma([raw.get((y * w + x) as usize).map_or(0, |v| level(*v))]));
    let s = scale(image, input);
    let size = |v: u32| ((v as f32 * s).round() as u32).max(1);
    resize(&map, size(image.0), size(image.1), FilterType::Triangle)
}

fn rows(map: &Map, r: usize, pick: fn(&[u16]) -> u16) -> Map {
    let (w, h) = (map.width() as usize, map.height());
    let row =
        |row: &[u16]| (0..w).map(|x| pick(&row[x.saturating_sub(r)..(x + r + 1).min(w)])).collect::<Vec<_>>();
    let out = map.as_raw().chunks_exact(w).flat_map(row).collect();
    Map::from_raw(w as u32, h, out).unwrap_or_default()
}

fn turn(map: &Map) -> Map {
    Map::from_fn(map.height(), map.width(), |x, y| *map.get_pixel(y, x))
}

fn both(map: Map, r: u32, pick: fn(&[u16]) -> u16) -> Map {
    match r {
        0 => map,
        r => turn(&rows(&turn(&rows(&map, r as usize, pick)), r as usize, pick)),
    }
}

fn mean(v: &[u16]) -> u16 {
    (v.iter().map(|x| u32::from(*x)).sum::<u32>() / v.len().max(1) as u32) as u16
}

pub fn adjust(map: &Map, depth: &Depth) -> Map {
    let (near, far, gamma) = (depth.near, depth.far, depth.gamma.max(0.01));
    let level = |v: u16| {
        let d = (f32::from(v) / 65535.0).powf(gamma);
        ((far + (near - far) * d).clamp(0.0, 1.0) * 65535.0).round() as u16
    };
    let mapped = Map::from_fn(map.width(), map.height(), |x, y| Luma([level(map.get_pixel(x, y)[0])]));
    let dilated = both(mapped, depth.dilate, |v| v.iter().copied().max().unwrap_or(0));
    both(dilated, depth.smooth, mean)
}

pub fn shrink(map: Map, edge: u32) -> Map {
    let long = map.width().max(map.height());
    if long <= edge {
        return map;
    }
    let size = |v: u32| ((u64::from(v) * u64::from(edge) / u64::from(long)) as u32).max(1);
    resize(&map, size(map.width()), size(map.height()), FilterType::Triangle)
}

const TURBO: [[f32; 6]; 3] = [
    [0.135_721_38, 4.615_392_6, -42.660_323, 132.131_08, -152.942_4, 59.286_38],
    [0.091_402_61, 2.194_188_4, 4.842_966_6, -14.185_033, 4.277_298_6, 2.829_566],
    [0.106_673_3, 12.641_946, -60.582_05, 110.362_77, -89.903_11, 27.348_25],
];

pub fn coloured(map: &Map, false_colour: bool) -> RgbImage {
    let paint = |v: u16| {
        let t = f32::from(v) / 65535.0;
        let channel = |c: &[f32; 6]| c.iter().rev().fold(0.0, |sum, k| sum * t + k).clamp(0.0, 1.0);
        let grey = [t; 3];
        let rgb = if false_colour { TURBO.each_ref().map(channel) } else { grey };
        Rgb(rgb.map(|c| (c * 255.0).round() as u8))
    };
    RgbImage::from_fn(map.width(), map.height(), |x, y| paint(map.get_pixel(x, y)[0]))
}
