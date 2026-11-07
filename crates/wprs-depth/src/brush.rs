use crate::Map;
use wprs_core::edit::Touch;

type Size = (usize, usize);

fn profile(radius: f32, falloff: f32, d: f32) -> f32 {
    let inner = radius * (1.0 - falloff);
    match d {
        d if d <= inner => 1.0,
        d if d >= radius => 0.0,
        d => {
            let t = (radius - d) / (radius - inner);
            t * t * (3.0 - 2.0 * t)
        }
    }
}

fn along(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx * dx + dy * dy;
    let t = if len > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn span(points: &[[f32; 2]], axis: usize, radius: f32, limit: usize) -> (usize, usize) {
    let low = points.iter().map(|p| p[axis]).fold(f32::MAX, f32::min) - radius;
    let high = points.iter().map(|p| p[axis]).fold(f32::MIN, f32::max) + radius;
    (low.floor().clamp(0.0, limit as f32) as usize, (high.ceil().clamp(0.0, limit as f32) as usize))
}

pub fn stamp((w, h): Size, touch: &Touch, mut apply: impl FnMut(usize, f32)) {
    let radius = touch.radius * w.max(h) as f32;
    let at: Vec<[f32; 2]> = touch.points.iter().map(|p| [p[0] * w as f32, p[1] * h as f32]).collect();
    let (x0, x1) = span(&at, 0, radius, w);
    let (y0, y1) = span(&at, 1, radius, h);
    let (bw, bh) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut weight = vec![0.0f32; bw * bh];
    let segments = at.windows(2).map(|s| (s[0], s[1])).chain((at.len() == 1).then(|| (at[0], at[0])));
    for (a, b) in segments {
        let (sx0, sx1) = span(&[a, b], 0, radius, w);
        let (sy0, sy1) = span(&[a, b], 1, radius, h);
        for y in sy0.max(y0)..sy1.min(y1) {
            for x in sx0.max(x0)..sx1.min(x1) {
                let d = along(a, b, [x as f32 + 0.5, y as f32 + 0.5]);
                let k = (y - y0) * bw + x - x0;
                weight[k] = weight[k].max(profile(radius, touch.falloff, d));
            }
        }
    }
    for (k, v) in weight.iter().enumerate().filter(|(_, v)| **v > 0.0) {
        apply((y0 + k / bw) * w + x0 + k % bw, *v);
    }
}

pub fn lift(plane: &mut [f32], size: Size, strokes: &[Touch]) {
    for t in strokes {
        stamp(size, t, |i, w| plane[i] = (plane[i] + t.by * w).clamp(0.0, 1.0));
    }
}

pub fn lift_map(map: &mut Map, strokes: &[Touch]) {
    let size = (map.width() as usize, map.height() as usize);
    for t in strokes {
        let raw: &mut [u16] = map;
        stamp(size, t, |i, w| {
            raw[i] = ((f32::from(raw[i]) / 65535.0 + t.by * w).clamp(0.0, 1.0) * 65535.0).round() as u16;
        });
    }
}

pub fn marks(size: Size, fills: &[Touch]) -> (Vec<bool>, Vec<bool>) {
    let (mut refill, mut protect) = (vec![false; size.0 * size.1], vec![false; size.0 * size.1]);
    for t in fills {
        stamp(size, t, |i, w| {
            if w >= 0.5 {
                (refill[i], protect[i]) = (t.by > 0.0, t.by < 0.0);
            }
        });
    }
    (refill, protect)
}
