use super::motion::lerp;
use super::{EmitShape, MAX_COLLIDERS2, MAX_EMITTERS2, MAX_POINTS, Surface, System};

pub const MAX_FIELD: usize = 16_384;

pub(super) fn fields<'a>(
    s: &System,
    image: &impl Fn(&str) -> Option<(u32, u32, &'a [u8])>,
    canvas: [f32; 2],
    points: &mut Vec<[f32; 2]>,
) -> [[f32; 4]; MAX_COLLIDERS2] {
    let mut out = [[0.0; 4]; MAX_COLLIDERS2];
    for (k, c) in s.collide.iter().take(MAX_COLLIDERS2).enumerate() {
        let Surface::Mask { texture, threshold } = &c.bounds else { continue };
        let Some((w, h, rgba)) = image(texture) else { continue };
        let ([cw, ch], d) = field(w, h, rgba, *threshold, canvas);
        out[k] = [4.0, points.len() as f32, cw as f32, ch as f32];
        points.extend(d.into_iter().map(|d| [d, 0.0]));
    }
    out
}

pub(super) fn shapes<'a>(
    s: &System,
    image: &impl Fn(&str) -> Option<(u32, u32, &'a [u8])>,
    [w, h]: [f32; 2],
    points: &mut Vec<[f32; 2]>,
) -> [[f32; 4]; MAX_EMITTERS2] {
    let mut ranges = [[0.0; 4]; MAX_EMITTERS2];
    for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
        let EmitShape::Mask { texture, threshold } = &em.shape else { continue };
        let Some((iw, ih, rgba)) = image(texture) else { continue };
        let stride = ((iw as f32 * ih as f32 / MAX_POINTS as f32).sqrt().ceil() as usize).max(1);
        let start = points.len();
        let at = |v: usize, n: u32, side: f32| (v as f32 + stride as f32 / 2.0) / n as f32 * side;
        for y in (0..ih as usize).step_by(stride) {
            for x in (0..iw as usize).step_by(stride) {
                let alpha = rgba.get((y * iw as usize + x) * 4 + 3).copied().unwrap_or(0);
                if f32::from(alpha) / 255.0 > threshold.0 {
                    points.push([at(x, iw, w), at(y, ih, h)]);
                }
            }
        }
        let cell = [stride as f32 / iw as f32 * w, stride as f32 / ih as f32 * h];
        ranges[e] = [start as f32, (points.len() - start) as f32, cell[0], cell[1]];
    }
    ranges
}

pub(super) fn reserve(s: &System, points: &mut Vec<[f32; 2]>) -> [[u32; 2]; MAX_EMITTERS2] {
    let mut out = [[0; 2]; MAX_EMITTERS2];
    for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
        if matches!(em.shape, EmitShape::Layer(_)) {
            out[e] = [points.len() as u32, 0];
            points.resize(points.len() + 2 * MAX_POINTS, [0.0; 2]);
        }
    }
    out
}

pub(super) fn field(
    iw: u32,
    ih: u32,
    rgba: &[u8],
    threshold: f32,
    canvas: [f32; 2],
) -> ([usize; 2], Vec<f32>) {
    let (iw, ih) = (iw.max(1) as usize, ih.max(1) as usize);
    let stride = ((iw * ih) as f32 / MAX_FIELD as f32).sqrt().ceil().max(1.0) as usize;
    let size = [iw.div_ceil(stride), ih.div_ceil(stride)];
    let cell = [canvas[0] / size[0] as f32, canvas[1] / size[1] as f32];
    let solid: Vec<bool> = (0..size[0] * size[1])
        .map(|c| {
            let x = (c % size[0] * stride + stride / 2).min(iw - 1);
            let y = (c / size[0] * stride + stride / 2).min(ih - 1);
            rgba.get((y * iw + x) * 4 + 3).is_some_and(|a| f32::from(*a) / 255.0 > threshold)
        })
        .collect();
    let (out, inside) = (seek(&solid, size, cell, true), seek(&solid, size, cell, false));
    let edge = cell[0].min(cell[1]) / 2.0;
    let d = solid.iter().enumerate().map(|(c, s)| if *s { edge - inside[c] } else { out[c] - edge });
    (size, d.collect())
}

fn seek(solid: &[bool], [w, h]: [usize; 2], cell: [f32; 2], to: bool) -> Vec<f32> {
    let far = [2 * w as i32 + 2, 2 * h as i32 + 2];
    let mut o: Vec<[i32; 2]> = solid.iter().map(|s| if *s == to { [0, 0] } else { far }).collect();
    let len = |v: [i32; 2]| (v[0] as f32 * cell[0]).hypot(v[1] as f32 * cell[1]);
    let look = |o: &mut Vec<[i32; 2]>, x: usize, y: usize, (dx, dy): (i32, i32)| {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
            let n = o[ny as usize * w + nx as usize];
            let c = [n[0] + dx, n[1] + dy];
            if len(c) < len(o[y * w + x]) {
                o[y * w + x] = c;
            }
        }
    };
    let ahead = [(-1, 0), (0, -1), (-1, -1), (1, -1)];
    let back = [(1, 0), (0, 1), (-1, 1), (1, 1)];
    for y in 0..h {
        (0..w).for_each(|x| ahead.into_iter().for_each(|d| look(&mut o, x, y, d)));
        (0..w).rev().for_each(|x| look(&mut o, x, y, (1, 0)));
    }
    for y in (0..h).rev() {
        (0..w).rev().for_each(|x| back.into_iter().for_each(|d| look(&mut o, x, y, d)));
        (0..w).for_each(|x| look(&mut o, x, y, (-1, 0)));
    }
    o.into_iter().map(len).collect()
}

pub(super) fn sample(
    points: &[[f32; 2]],
    a: [f32; 4],
    canvas: [f32; 2],
    pos: [f32; 2],
) -> Option<(f32, [f32; 2])> {
    let (start, w, h) = (a[1] as usize, a[2], a[3]);
    let (fx, fy) = (pos[0] / canvas[0] * w - 0.5, pos[1] / canvas[1] * h - 0.5);
    if !(fx > -0.5 && fy > -0.5 && fx < w - 0.5 && fy < h - 0.5) {
        return None;
    }
    let (x0, y0) = (fx.floor().clamp(0.0, w - 1.0), fy.floor().clamp(0.0, h - 1.0));
    let (x1, y1) = ((x0 + 1.0).min(w - 1.0), (y0 + 1.0).min(h - 1.0));
    let (u, v) = ((fx - x0).clamp(0.0, 1.0), (fy - y0).clamp(0.0, 1.0));
    let at = |x: f32, y: f32| points.get(start + (y * w + x) as usize).map_or(1e9, |p| p[0]);
    let (d00, d10, d01, d11) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    let d = lerp(lerp(d00, d10, u), lerp(d01, d11, u), v);
    let g = [lerp(d10 - d00, d11 - d01, v) * w / canvas[0], lerp(d01 - d00, d11 - d10, u) * h / canvas[1]];
    Some((d, g))
}
