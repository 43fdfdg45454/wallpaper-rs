use wprs_scene::Affine;

const CROP: f32 = 0.8;

pub(crate) fn window(uv: Affine, m: Affine, reach: f32) -> [f32; 4] {
    let corners = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]].map(|p| uv.apply(p));
    let lo = corners.iter().fold([f32::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
    let hi = corners.iter().fold([f32::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
    let grow = [reach / m.0[0].hypot(m.0[1]).max(1e-6), reach / m.0[2].hypot(m.0[3]).max(1e-6)];
    [lo[0] - grow[0], lo[1] - grow[1], hi[0] + grow[0], hi[1] + grow[1]]
}

pub(crate) fn cropped(
    m: Affine,
    (w, h): (u32, u32),
    window: Option<[f32; 4]>,
) -> (Affine, (u32, u32), [f32; 4]) {
    let Some([x0, y0, x1, y1]) = window.filter(|v| v.iter().all(|c| c.is_finite())) else {
        return (m, (w, h), [0.0; 4]);
    };
    let edge = |v: f32, n: u32, out: f32| (v * n as f32 + out).clamp(0.0, n as f32);
    let (a, b) = (edge(x0, w, -1.0).floor(), edge(y0, h, -1.0).floor());
    let (c, d) = (edge(x1, w, 1.0).ceil(), edge(y1, h, 1.0).ceil());
    if c <= a || d <= b || (c - a) * (d - b) > CROP * (w * h) as f32 {
        return (m, (w, h), [0.0; 4]);
    }
    let crop = [a / w as f32, b / h as f32, (c - a) / w as f32, (d - b) / h as f32];
    let inner = Affine([crop[2], 0.0, 0.0, crop[3], crop[0], crop[1]]);
    (m.then(inner), ((c - a) as u32, (d - b) as u32), crop)
}

pub(crate) fn projection(j: usize, m: Affine, size: (u32, u32)) -> usize {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    (j, m.0.map(f32::to_bits), size).hash(&mut h);
    h.finish() as usize | 1 << (usize::BITS - 1)
}
