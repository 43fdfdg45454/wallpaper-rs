use crate::Map;
use image::imageops::{FilterType, resize};
use image::{Luma, RgbImage};

pub const RADIUS: usize = 8;
pub const EPSILON: f32 = 0.002;

pub type Size = (usize, usize);

fn sums(line: impl Iterator<Item = f32>, out: &mut [f32], r: usize) {
    let mut running = vec![0.0f64];
    running.extend(line.scan(0.0f64, |s, v| {
        *s += f64::from(v);
        Some(*s)
    }));
    let n = out.len();
    for (i, o) in out.iter_mut().enumerate() {
        let (lo, hi) = (i.saturating_sub(r), (i + r + 1).min(n));
        *o = ((running[hi] - running[lo]) / (hi - lo) as f64) as f32;
    }
}

pub fn boxed(v: &[f32], (w, h): Size, r: usize) -> Vec<f32> {
    let mut rows = vec![0.0; v.len()];
    for (src, dst) in v.chunks_exact(w).zip(rows.chunks_exact_mut(w)) {
        sums(src.iter().copied(), dst, r);
    }
    let mut out = vec![0.0; v.len()];
    let mut column = vec![0.0; h];
    for x in 0..w {
        sums((0..h).map(|y| rows[y * w + x]), &mut column, r);
        column.iter().enumerate().for_each(|(y, c)| out[y * w + x] = *c);
    }
    out
}

fn product(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(a, b)| a * b).collect()
}

type Guide<'a> = [&'a [f32]; 3];

fn solve(s: [f32; 6], v: [f32; 3]) -> [f32; 3] {
    let [rr, rg, rb, gg, gb, bb] = s;
    let c = [gg * bb - gb * gb, gb * rb - rg * bb, rg * gb - gg * rb];
    let inverse = [
        [c[0], c[1], c[2]],
        [c[1], rr * bb - rb * rb, rg * rb - rr * gb],
        [c[2], rg * rb - rr * gb, rr * gg - rg * rg],
    ];
    let det = rr * c[0] + rg * c[1] + rb * c[2];
    inverse.map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum::<f32>() / det)
}

pub fn filter(guide: Guide, input: &[f32], size: Size, (r, eps): (usize, f32)) -> Vec<f32> {
    let n = input.len();
    let mean_i = guide.map(|g| boxed(g, size, r));
    let mean_p = boxed(input, size, r);
    let pairs = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    let corr = pairs.map(|(i, j)| boxed(&product(guide[i], guide[j]), size, r));
    let cross = [0, 1, 2].map(|i| boxed(&product(guide[i], input), size, r));
    let (mut a, mut b) = ([vec![0.0; n], vec![0.0; n], vec![0.0; n]], vec![0.0; n]);
    for k in 0..n {
        let mut sigma =
            [0, 1, 2, 3, 4, 5].map(|q| corr[q][k] - mean_i[pairs[q].0][k] * mean_i[pairs[q].1][k]);
        [0, 3, 5].iter().for_each(|d| sigma[*d] += eps);
        let cov = [0, 1, 2].map(|i| cross[i][k] - mean_i[i][k] * mean_p[k]);
        let slope = solve(sigma, cov);
        (0..3).for_each(|c| a[c][k] = slope[c]);
        b[k] = mean_p[k] - (0..3).map(|c| slope[c] * mean_i[c][k]).sum::<f32>();
    }
    let (a, b) = (a.map(|a| boxed(&a, size, r)), boxed(&b, size, r));
    (0..n).map(|k| ((0..3).map(|c| a[c][k] * guide[c][k]).sum::<f32>() + b[k]).clamp(0.0, 1.0)).collect()
}

pub fn planes(image: &RgbImage) -> [Vec<f32>; 3] {
    [0, 1, 2].map(|c| image.pixels().map(|p| f32::from(p[c]) / 255.0).collect())
}

pub fn refine(depth: &Map, guide: &RgbImage) -> Vec<f32> {
    let (w, h) = guide.dimensions();
    let big = resize(depth, w, h, FilterType::Triangle);
    let input: Vec<f32> = big.pixels().map(|Luma([v])| f32::from(*v) / 65535.0).collect();
    let [r, g, b] = planes(guide);
    filter([&r, &g, &b], &input, (w as usize, h as usize), (RADIUS, EPSILON))
}
