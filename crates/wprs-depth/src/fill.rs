use crate::guided::{Size, boxed};
use image::{Rgb, RgbImage};

pub const BLUR: usize = 2;

struct Level {
    size: Size,
    color: Vec<[f32; 3]>,
    cover: Vec<f32>,
}

fn shrink(l: &Level) -> Level {
    let size = (l.size.0.div_ceil(2), l.size.1.div_ceil(2));
    let (mut color, mut cover) = (vec![[0.0; 3]; size.0 * size.1], vec![0.0; size.0 * size.1]);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let (mut sum, mut weight) = ([0.0; 3], 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = ((2 * x + dx).min(l.size.0 - 1), (2 * y + dy).min(l.size.1 - 1));
                let k = sy * l.size.0 + sx;
                (0..3).for_each(|c| sum[c] += l.color[k][c] * l.cover[k]);
                weight += l.cover[k];
            }
            let i = y * size.0 + x;
            color[i] = if weight > 0.0 { sum.map(|s| s / weight) } else { [0.0; 3] };
            cover[i] = weight / 4.0;
        }
    }
    Level { size, color, cover }
}

fn grow(coarse: &[[f32; 3]], from: Size, to: Size) -> Vec<[f32; 3]> {
    let at = |v: f32, n: usize| {
        let p = ((v + 0.5) / 2.0 - 0.5).clamp(0.0, (n - 1) as f32);
        (p.floor() as usize, (p.floor() as usize + 1).min(n - 1), p.fract())
    };
    let mut out = Vec::with_capacity(to.0 * to.1);
    for y in 0..to.1 {
        let (y0, y1, fy) = at(y as f32, from.1);
        for x in 0..to.0 {
            let (x0, x1, fx) = at(x as f32, from.0);
            let sample = |sx: usize, sy: usize| coarse[sy * from.0 + sx];
            let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * t);
            out.push(mix(
                mix(sample(x0, y0), sample(x1, y0), fx),
                mix(sample(x0, y1), sample(x1, y1), fx),
                fy,
            ));
        }
    }
    out
}

pub fn push_pull(image: &RgbImage, known: &[bool]) -> RgbImage {
    let size = (image.width() as usize, image.height() as usize);
    let color = image.pixels().map(|p| p.0.map(f32::from)).collect();
    let cover = known.iter().map(|k| f32::from(u8::from(*k))).collect();
    let mut levels = vec![Level { size, color, cover }];
    while levels.last().is_some_and(|l| l.size.0 > 1 || l.size.1 > 1) {
        levels.push(shrink(&levels[levels.len() - 1]));
    }
    let mut result: Vec<[f32; 3]> = Vec::new();
    for l in levels.iter().rev() {
        let below = if result.is_empty() {
            vec![[0.0; 3]; l.size.0 * l.size.1]
        } else {
            grow(&result, (l.size.0.div_ceil(2), l.size.1.div_ceil(2)), l.size)
        };
        result = (0..l.cover.len())
            .map(|k| {
                let t = l.cover[k].min(1.0);
                [0, 1, 2].map(|c| l.color[k][c] * t + below[k][c] * (1.0 - t))
            })
            .collect();
    }
    let pixels = result.iter().flat_map(|p| p.map(|c| c.round().clamp(0.0, 255.0) as u8));
    RgbImage::from_raw(image.width(), image.height(), pixels.collect()).unwrap_or_else(|| image.clone())
}

pub fn blur(image: &mut RgbImage, keep: &[bool]) {
    let size = (image.width() as usize, image.height() as usize);
    let planes: Vec<Vec<f32>> = (0..3).map(|c| image.pixels().map(|p| f32::from(p[c])).collect()).collect();
    let blurred: Vec<Vec<f32>> = planes.iter().map(|p| boxed(p, size, BLUR)).collect();
    for (k, p) in image.pixels_mut().enumerate().filter(|(k, _)| !keep[*k]) {
        *p = Rgb([0, 1, 2].map(|c| blurred[c][k].round() as u8));
    }
}

pub fn simple(image: &RgbImage, known: &[bool]) -> RgbImage {
    let mut filled = push_pull(image, known);
    blur(&mut filled, known);
    filled
}
