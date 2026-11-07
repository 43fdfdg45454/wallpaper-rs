use crate::guided::{Size, boxed};
use anyhow::Result;
use image::imageops::{FilterType, resize};
use image::{GrayImage, Luma, Rgb, RgbImage};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

pub const CROP: u32 = 512;
pub const MOST_RUNS: usize = 8;
pub const FEATHER: usize = 2;
pub const LEAST: usize = 24;

pub type Inpaint<'a> = dyn FnMut(&RgbImage, &GrayImage) -> Result<RgbImage> + 'a;

fn tiles((w, h): Size, side: usize, band: &[bool]) -> Vec<(usize, usize, usize, usize)> {
    let (cw, ch) = (side.min(w), side.min(h));
    let stride = (side * 3 / 4).max(1);
    let starts = |n: usize, c: usize| -> Vec<usize> {
        let mut s: Vec<usize> = (0..n.saturating_sub(c) + 1).step_by(stride).collect();
        if s.last().is_none_or(|l| l + c < n) {
            s.push(n - c);
        }
        s
    };
    let mut found = Vec::new();
    for &y in &starts(h, ch) {
        for &x in &starts(w, cw) {
            let masked: usize =
                (y..y + ch).map(|j| band[j * w + x..j * w + x + cw].iter().filter(|b| **b).count()).sum();
            if masked >= LEAST {
                found.push((x, y, cw, ch));
            }
        }
    }
    found
}

pub fn plan(size: Size, band: &[bool]) -> Vec<(usize, usize, usize, usize)> {
    let mut side = CROP as usize;
    loop {
        let found = tiles(size, side, band);
        if found.len() <= MOST_RUNS || side >= size.0.max(size.1) {
            return found;
        }
        side *= 2;
    }
}

pub fn lama(image: &mut RgbImage, band: &[bool], inpaint: &mut Inpaint<'_>, stop: &AtomicBool) -> Result<()> {
    let size = (image.width() as usize, image.height() as usize);
    let flat: Vec<f32> = band.iter().map(|b| f32::from(u8::from(*b))).collect();
    let weight = boxed(&flat, size, FEATHER);
    for (x, y, w, h) in plan(size, band) {
        anyhow::ensure!(!stop.load(Relaxed), "cancelled");
        let crop = image::imageops::crop_imm(image, x as u32, y as u32, w as u32, h as u32).to_image();
        let mask = GrayImage::from_fn(w as u32, h as u32, |i, j| {
            Luma([if band[(y + j as usize) * size.0 + x + i as usize] { 255 } else { 0 }])
        });
        let input = resize(&crop, CROP, CROP, FilterType::Triangle);
        let hole = resize(&mask, CROP, CROP, FilterType::Triangle);
        let hole =
            GrayImage::from_fn(CROP, CROP, |i, j| Luma([if hole.get_pixel(i, j)[0] > 0 { 255 } else { 0 }]));
        let out = resize(&inpaint(&input, &hole)?, w as u32, h as u32, FilterType::Triangle);
        for j in 0..h {
            for i in 0..w {
                let t = weight[(y + j) * size.0 + x + i];
                if t > 0.0 {
                    let (a, b) = (
                        image.get_pixel(x as u32 + i as u32, y as u32 + j as u32).0,
                        out.get_pixel(i as u32, j as u32).0,
                    );
                    let mixed =
                        [0, 1, 2].map(|c| (f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t).round() as u8);
                    image.put_pixel(x as u32 + i as u32, y as u32 + j as u32, Rgb(mixed));
                }
            }
        }
    }
    Ok(())
}
