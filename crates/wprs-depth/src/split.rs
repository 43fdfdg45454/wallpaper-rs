use crate::guided::{Size, boxed};

const BINS: usize = 1024;
const SMALLEST: f64 = 0.002;
const ROUNDS: usize = 40;
pub const CUT_PIXELS: usize = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub cuts: Vec<f32>,
    pub depths: Vec<f32>,
}

impl Split {
    pub fn count(&self) -> usize {
        self.depths.len()
    }

    pub fn index(&self, depth: f32) -> usize {
        self.cuts.partition_point(|c| *c <= depth)
    }
}

fn level(bin: usize) -> f32 {
    bin as f32 / (BINS - 1) as f32
}

fn nearest(centres: &[f32], v: f32) -> usize {
    let k = centres.partition_point(|c| *c < v);
    match k {
        0 => 0,
        k if k == centres.len() => k - 1,
        k if v - centres[k - 1] <= centres[k] - v => k - 1,
        k => k,
    }
}

fn groups(hist: &[u64], centres: &[f32]) -> Vec<(f64, u64)> {
    let mut sums = vec![(0.0, 0); centres.len()];
    for (bin, &n) in hist.iter().enumerate().filter(|(_, n)| **n > 0) {
        let k = nearest(centres, level(bin));
        sums[k] = (sums[k].0 + n as f64 * f64::from(level(bin)), sums[k].1 + n);
    }
    sums
}

pub fn split(depth: &[f32], count: usize) -> Split {
    let mut hist = vec![0u64; BINS];
    depth.iter().for_each(|d| hist[(d.clamp(0.0, 1.0) * (BINS - 1) as f32).round() as usize] += 1);
    let total = depth.len().max(1) as u64;
    let first = hist.iter().position(|n| *n > 0).map_or(0.0, level);
    let last = hist.iter().rposition(|n| *n > 0).map_or(0.0, level);
    let (lo, hi) = (first, last);
    let n = count.max(1);
    let mut centres: Vec<f32> = (0..n).map(|k| lo + (hi - lo) * (k as f32 + 0.5) / n as f32).collect();
    for _ in 0..ROUNDS {
        let next: Vec<f32> = groups(&hist, &centres)
            .iter()
            .zip(&centres)
            .map(|(&(sum, n), c)| if n > 0 { (sum / n as f64) as f32 } else { *c })
            .collect();
        if next == centres {
            break;
        }
        centres = next;
    }
    loop {
        let found = groups(&hist, &centres);
        let weakest = found.iter().enumerate().min_by_key(|(_, g)| g.1).map(|(k, g)| (k, g.1));
        match weakest {
            Some((k, n)) if centres.len() > 1 && (n as f64) < SMALLEST * total as f64 => {
                centres.remove(k);
            }
            _ => {
                let depths = found
                    .iter()
                    .zip(&centres)
                    .map(|(&(s, n), c)| if n > 0 { (s / n as f64) as f32 } else { *c });
                let depths: Vec<f32> = depths.collect();
                let cuts = depths.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect();
                return Split { cuts, depths };
            }
        }
    }
}

pub fn indices(depth: &[f32], split: &Split) -> Vec<u8> {
    depth.iter().map(|d| split.index(*d) as u8).collect()
}

pub fn mask(index: &[u8], layer: usize) -> Vec<bool> {
    index.iter().map(|i| usize::from(*i) == layer).collect()
}

pub fn soft(mask: &[bool], size: Size) -> Vec<f32> {
    let flat: Vec<f32> = mask.iter().map(|m| f32::from(u8::from(*m))).collect();
    boxed(&flat, size, CUT_PIXELS)
}

pub fn distance(mask: &[bool], (w, h): Size) -> Vec<f32> {
    let far = (w + h) as f32 * 2.0;
    let mut d: Vec<f32> = mask.iter().map(|m| if *m { 0.0 } else { far }).collect();
    let diagonal = std::f32::consts::SQRT_2;
    let relax = |d: &mut Vec<f32>, x: usize, y: usize, neighbours: &[(isize, isize, f32)]| {
        for &(dx, dy, cost) in neighbours {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let via = d[ny as usize * w + nx as usize] + cost;
                d[y * w + x] = d[y * w + x].min(via);
            }
        }
    };
    let ahead = [(-1, 0, 1.0), (0, -1, 1.0), (-1, -1, diagonal), (1, -1, diagonal)];
    let behind = [(1, 0, 1.0), (0, 1, 1.0), (1, 1, diagonal), (-1, 1, diagonal)];
    (0..h).for_each(|y| (0..w).for_each(|x| relax(&mut d, x, y, &ahead)));
    (0..h).rev().for_each(|y| (0..w).rev().for_each(|x| relax(&mut d, x, y, &behind)));
    d
}
