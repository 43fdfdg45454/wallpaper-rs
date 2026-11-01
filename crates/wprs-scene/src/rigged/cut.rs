use super::delaunay::{inside, near, outline, triangulate};
use super::sketch::Sketch;
use std::collections::HashMap;

pub const MIN_CELLS: u32 = 2;
pub const MAX_CELLS: u32 = 64;
const SOLID: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    Grid,
    Outline,
}

pub(super) struct Mask<'a> {
    pub alpha: &'a [u8],
    pub size: [u32; 2],
}

impl Mask<'_> {
    pub fn solid(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        let [w, h] = self.size;
        let (x0, y0) = ((x0.floor().max(0.0) as u32).min(w), (y0.floor().max(0.0) as u32).min(h));
        let (x1, y1) = ((x1.ceil().max(0.0) as u32).min(w), (y1.ceil().max(0.0) as u32).min(h));
        (y0..y1).any(|y| (x0..x1).any(|x| self.alpha[(y * w + x) as usize] > SOLID))
    }
}

impl Sketch {
    pub fn cut(alpha: &[u8], size: [u32; 2], cut: Cut, cells: u32) -> Sketch {
        let mask = Mask { alpha, size };
        let fits = alpha.len() == (size[0] as usize) * (size[1] as usize) && size[0] > 0 && size[1] > 0;
        let opaque = !fits || !mask.solid(0.0, 0.0, size[0] as f32, size[1] as f32);
        let cells = cells.clamp(MIN_CELLS, MAX_CELLS);
        let mut sketch = match cut {
            Cut::Outline if !opaque => outlined(&mask, cells),
            _ => grid(&mask, cells, opaque),
        };
        if sketch.triangles.is_empty() {
            sketch = grid(&mask, cells, true);
        }
        sketch.size = size.map(|v| v.max(1) as f32);
        let points = &sketch.points;
        for t in &mut sketch.triangles {
            let [p, q, r] = t.map(|i| points[i as usize]);
            if (q[0] - p[0]) * (r[1] - p[1]) - (r[0] - p[0]) * (q[1] - p[1]) > 0.0 {
                t.swap(1, 2);
            }
        }
        sketch
    }
}

fn step(size: [u32; 2], cells: u32) -> f32 {
    size[0].max(size[1]).max(1) as f32 / cells as f32
}

fn grid(mask: &Mask, cells: u32, all: bool) -> Sketch {
    let (s, [w, h]) = (step(mask.size, cells), mask.size.map(|v| v as f32));
    let (nx, ny) = ((w / s).ceil().max(1.0) as u32, (h / s).ceil().max(1.0) as u32);
    let corner = |i: u32, j: u32| [(i as f32 * s).min(w), (j as f32 * s).min(h)];
    let mut sketch = Sketch::default();
    let mut seen: HashMap<(u32, u32), u32> = HashMap::new();
    let mut point = |sketch: &mut Sketch, i: u32, j: u32| {
        *seen.entry((i, j)).or_insert_with(|| {
            sketch.points.push(corner(i, j));
            sketch.points.len() as u32 - 1
        })
    };
    for j in 0..ny {
        for i in 0..nx {
            let ([x0, y0], [x1, y1]) = (corner(i, j), corner(i + 1, j + 1));
            if !all && !mask.solid(x0, y0, x1, y1) {
                continue;
            }
            let [a, b, c, d] =
                [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)].map(|(x, y)| point(&mut sketch, x, y));
            sketch.triangles.extend([[a, b, c], [a, c, d]]);
        }
    }
    sketch
}

fn outlined(mask: &Mask, cells: u32) -> Sketch {
    let s = step(mask.size, cells);
    let ring = outline(mask, s / 4.0, s * 0.75);
    if ring.len() < 3 {
        return Sketch::default();
    }
    let [w, h] = mask.size.map(|v| v as f32);
    let mut points = ring.clone();
    let (nx, ny) = ((w / s).ceil() as u32, (h / s).ceil() as u32);
    for j in 0..=ny {
        for i in 0..=nx {
            let p = [(i as f32 * s).min(w), (j as f32 * s).min(h)];
            if inside(&ring, p) && near(&ring, p) > s * 0.4 {
                points.push(p);
            }
        }
    }
    let triangles = triangulate(&points)
        .into_iter()
        .filter(|t| {
            let c = (0..2).map(|k| t.iter().map(|i| points[*i as usize][k]).sum::<f32>() / 3.0);
            let c: Vec<f32> = c.collect();
            inside(&ring, [c[0], c[1]])
        })
        .collect();
    Sketch { points, triangles, ..Sketch::default() }
}
