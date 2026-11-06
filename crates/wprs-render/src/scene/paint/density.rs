use super::super::item::Rect;
use super::Painter;
use wprs_scene::{Affine, Content};

const LEAST: f32 = 0.125;
const LOOSE: f32 = 1.5;
const WHOLE: f32 = 0.995;

impl Painter<'_, '_, '_> {
    fn texels(&self, i: usize) -> Option<[f32; 2]> {
        let d = &self.frame.draws[i];
        if d.effects.is_empty()
            || self.backdrop.is_some()
            || self.lean(i).is_some()
            || self.flat.get().is_some()
        {
            return None;
        }
        match &d.content {
            Content::Image { .. } | Content::Video { .. } => self.natural(i),
            Content::Mesh { texture: Some(t), .. } => self.images.get(t.as_ref()).map(|image| image.1),
            _ => None,
        }
    }

    pub(crate) fn fitted(&self, i: usize, rect: Rect) -> (Rect, f32, (u32, u32)) {
        let plain = rect.bucket();
        let (Some([tw, th]), Some((size, _))) = (self.texels(i), self.geometry(i)) else {
            return (plain, 1.0, plain.size());
        };
        let m = self.local(i, size).0;
        let shown = [m[0].hypot(m[1]), m[2].hypot(m[3])];
        let dense = (tw / shown[0].max(1e-6)).max(th / shown[1].max(1e-6)) * self.frame.quality.density();
        if dense >= WHOLE {
            return (plain, 1.0, plain.size());
        }
        let dense = dense.max(LEAST);
        let step = [shown[0] / tw, shown[1] / th];
        let image = matches!(self.frame.draws[i].content, Content::Image { .. } | Content::Video { .. });
        let square = m[1].abs() < 1e-4 && m[2].abs() < 1e-4 && (step[0] / step[1] - 1.0).abs() < 1e-3;
        let full = self.frame.quality.density() >= 1.0;
        if !image || !square || !full {
            let dense = if full { (dense * LOOSE).min(1.0) } else { dense };
            return (plain, dense, self.dense_at(plain.size(), dense));
        }
        let s = step[0];
        let x0 = m[4] + ((rect.x - m[4]) / s).floor() * s;
        let y0 = m[5] + ((rect.y - m[5]) / s).floor() * s;
        let n = |from: f32, to: f32| ((to - from) / s - 1e-3).ceil().max(1.0);
        let grid = Rect { x: x0, y: y0, w: n(x0, rect.x + rect.w), h: n(y0, rect.y + rect.h) }.bucket();
        let fit = Rect { w: grid.w * s, h: grid.h * s, ..grid };
        (fit, 1.0 / s, grid.size())
    }

    pub(crate) fn sharp(&self, j: usize, m: Affine, (w, h): (u32, u32)) -> (u32, u32) {
        let texels = match &self.frame.draws[j].content {
            Content::Image { .. } => self.natural(j),
            Content::Mesh { texture: Some(t), .. } => self.images.get(t.as_ref()).map(|image| image.1),
            _ => None,
        };
        let (Some([tw, th]), Some((size, _))) = (texels, self.geometry(j)) else { return (w, h) };
        let (l, m) = (self.local(j, size).0, m.0);
        if l[1] != 0.0 || l[2] != 0.0 || m[1] != 0.0 || m[2] != 0.0 {
            return (w, h);
        }
        let q = self.frame.quality.density();
        let side = |t: f32, need: f32, shown: f32, n: u32| {
            ((t * (need / shown).abs() * q - 1e-3).ceil().max(1.0) as u32).min(n)
        };
        (side(tw, m[0], l[0], w), side(th, m[3], l[3], h))
    }

    pub(crate) fn dense(&self, size: (u32, u32)) -> (u32, u32) {
        self.dense_at(size, self.density)
    }

    fn dense_at(&self, (w, h): (u32, u32), dense: f32) -> (u32, u32) {
        let side = |v: u32| ((v as f32 * dense - 1e-3).ceil() as u32).max(1);
        (side(w), side(h))
    }
}
