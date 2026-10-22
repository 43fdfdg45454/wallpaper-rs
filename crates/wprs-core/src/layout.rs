use crate::{Fit, UvRect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    pub bezel_mm: f64,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub monitors: BTreeMap<String, Placed>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Placed {
    pub mm: [f64; 4],
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub px_per_mm: f64,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub mm: [f64; 4],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Place {
    #[default]
    Fit,
    Crop([f32; 4]),
    Span {
        monitors: Vec<Monitor>,
        output: String,
        center: [f32; 2],
        zoom: f32,
    },
}

fn size_mm((w, h): (u32, u32), s: &Screen) -> (f64, f64) {
    let fallback = 96.0 / 25.4 * if s.scale.is_finite() { s.scale.clamp(0.25, 8.0) } else { 1.0 };
    let density = Some(s.px_per_mm).filter(|d| (2.0..=20.0).contains(d)).unwrap_or(fallback);
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    (w / density, h / density)
}

fn beside(m: &Monitor, p: &Screen, s: &Screen, (w, h): (f64, f64), gap: f64) -> ([f64; 4], bool) {
    let [x, y, pw, ph] = m.mm;
    let center = |a: i32, l: u32| f64::from(a) + f64::from(l) / 2.0;
    let cx = x + pw / 2.0 + (center(s.x, s.width) - center(p.x, p.width)) * pw / f64::from(p.width.max(1));
    let cy = y + ph / 2.0 + (center(s.y, s.height) - center(p.y, p.height)) * ph / f64::from(p.height.max(1));
    let (cx, cy) = (cx - w / 2.0, cy - h / 2.0);
    let end = |a: i32, l: u32| i64::from(a) + i64::from(l);
    let rows = i64::from(s.y) < end(p.y, p.height) && i64::from(p.y) < end(s.y, s.height);
    let columns = i64::from(s.x) < end(p.x, p.width) && i64::from(p.x) < end(s.x, s.width);
    let sides = [
        (rows && i64::from(s.x) == end(p.x, p.width), [x + pw + gap, cy, w, h]),
        (rows && end(s.x, s.width) == i64::from(p.x), [x - gap - w, cy, w, h]),
        (columns && i64::from(s.y) == end(p.y, p.height), [cx, y + ph + gap, w, h]),
        (columns && end(s.y, s.height) == i64::from(p.y), [cx, y - gap - h, w, h]),
    ];
    sides.into_iter().find(|(touching, _)| *touching).map_or(([cx, cy, w, h], false), |(_, mm)| (mm, true))
}

impl Layout {
    pub const MAX_BEZEL_MM: f64 = 100.0;

    pub fn is_default(&self) -> bool {
        *self == Layout::default()
    }

    pub fn arrange<'a>(
        &self,
        screens: impl IntoIterator<Item = (&'a str, (u32, u32), &'a Screen)>,
    ) -> Vec<Monitor> {
        let mut screens: Vec<_> = screens.into_iter().collect();
        screens.sort_by_key(|(id, _, s)| (s.x, s.y, *id));
        let gap =
            2.0 * if self.bezel_mm.is_finite() { self.bezel_mm.clamp(0.0, Self::MAX_BEZEL_MM) } else { 0.0 };
        let mut placed: Vec<(Monitor, &Screen)> = Vec::new();
        for (id, px, s) in screens {
            let size = size_mm(px, s);
            let fixed =
                [s.identity.as_str(), id].into_iter().find_map(|k| self.monitors.get(k)).map(|p| p.mm);
            let fixed = fixed.filter(|mm| mm.iter().all(|v| v.is_finite()) && mm[2] > 0.0 && mm[3] > 0.0);
            let near = placed.iter().map(|(m, p)| beside(m, p, s, size, gap));
            let mut near: Vec<_> = near.collect();
            near.sort_by_key(|(_, touching)| !touching);
            let mm = fixed.or(near.first().map(|(mm, _)| *mm)).unwrap_or([0.0, 0.0, size.0, size.1]);
            placed.push((Monitor { id: id.into(), mm }, s));
        }
        placed.into_iter().map(|(m, _)| m).collect()
    }
}

pub fn span(monitors: &[Monitor], image: (u32, u32), zoom: f64, center: [f64; 2]) -> Vec<(String, UvRect)> {
    let lo = |i: usize| monitors.iter().map(|m| m.mm[i]).fold(f64::INFINITY, f64::min);
    let hi = |i: usize| monitors.iter().map(|m| m.mm[i] + m.mm[i + 2]).fold(f64::NEG_INFINITY, f64::max);
    let (x0, y0, w, h) = (lo(0), lo(1), hi(0) - lo(0), hi(1) - lo(1));
    let (iw, ih) = (f64::from(image.0.max(1)), f64::from(image.1.max(1)));
    let mm_per_px = (w / iw).max(h / ih) * zoom.max(1.0);
    let (vw, vh) = ((w / mm_per_px / iw).min(1.0), (h / mm_per_px / ih).min(1.0));
    let (ox, oy) = ((center[0] - vw / 2.0).clamp(0.0, 1.0 - vw), (center[1] - vh / 2.0).clamp(0.0, 1.0 - vh));
    let uv = |m: &Monitor| UvRect {
        offset: [ox + (m.mm[0] - x0) / w * vw, oy + (m.mm[1] - y0) / h * vh].map(|v| v as f32),
        scale: [m.mm[2] / w * vw, m.mm[3] / h * vh].map(|v| v as f32),
        repeat: false,
    };
    monitors.iter().map(|m| (m.id.clone(), uv(m))).collect()
}

impl Place {
    pub fn uv(&self, fit: Fit, image: (u32, u32), target: (u32, u32)) -> UvRect {
        let fill = |image| UvRect::compute(Fit::Fill, image, target);
        match self {
            Place::Fit => UvRect::compute(fit, image, target),
            Place::Crop([x, y, w, h]) => {
                let inner = fill(((image.0 as f32 * w).round() as u32, (image.1 as f32 * h).round() as u32));
                let (offset, scale) = (
                    [x + inner.offset[0] * w, y + inner.offset[1] * h],
                    [inner.scale[0] * w, inner.scale[1] * h],
                );
                UvRect { offset, scale, repeat: false }
            }
            Place::Span { monitors, output, center, zoom } => {
                let all = span(monitors, image, f64::from(*zoom), center.map(f64::from));
                all.into_iter().find(|(id, _)| id == output).map_or_else(|| fill(image), |(_, uv)| uv)
            }
        }
    }
}
