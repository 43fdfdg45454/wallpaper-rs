use super::item::{Item, Rect, invert};
use super::paint::{Painter, Target};
use super::pipes::Mode;
use super::{Op, Src};
use wprs_scene::{Affine, Kind};

impl Painter<'_, '_, '_> {
    pub(crate) fn captures(&self, i: usize) -> Option<(Option<[f32; 2]>, bool)> {
        match self.frame.draws[i].layer.kind {
            Kind::Adjustment { size, redraw, .. } if size.is_some() || redraw => Some((size, redraw)),
            _ => None,
        }
    }

    pub(crate) fn capture(&mut self, i: usize, target: Target, (size, redraw): (Option<[f32; 2]>, bool)) {
        let frame = self.frame;
        let d = &frame.draws[i];
        if (d.effects.is_empty() && !redraw) || (!d.effects.is_empty() && self.idle(i)) {
            return;
        }
        let canvas = frame.canvas;
        let half = canvas.map(|v| v / 2.0);
        let (grab, put) = match size {
            Some(size) => {
                let at = Affine::translate([d.world.0[4], d.world.0[5]]);
                let shift = Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]);
                (self.screen.then(at).then(shift).then(Affine::scale(size)), self.local(i, size))
            }
            None => {
                let around = Affine::translate(half).then(d.world).then(Affine::translate(half.map(|v| -v)));
                (self.stage, self.screen.then(around).then(Affine::scale(canvas)))
            }
        };
        let [gx, gy, gw, gh] = [grab.0[4], grab.0[5], grab.0[0], grab.0[3]];
        let (x0, y0) = (gx.min(gx + gw).floor(), gy.min(gy + gh).floor());
        let (x1, y1) = (gx.max(gx + gw).ceil(), gy.max(gy + gh).ceil());
        let rect = Rect { x: x0, y: y0, w: (x1 - x0).max(1.0), h: (y1 - y0).max(1.0) };
        let Some(src) = self.under(rect, target) else { return };
        let out = match d.effects.is_empty() {
            true => src,
            false => self.effects(i, src, rect, invert(grab).then(rect.affine())),
        };
        if out != src {
            self.release(src);
        }
        let uv = [(gx - rect.x) / rect.w, (gy - rect.y) / rect.h, gw / rect.w, gh / rect.h];
        let mut item = Item { quad: [0.0, 0.0, 1.0, 1.0], uv, color: [d.opacity; 4], ..Item::default() };
        let m = if redraw { self.map(i, put) } else { super::map::Map::Flat(grab) };
        let Some(bounds) = m.bounds(item.quad).and_then(|r| r.pixels(target.rect)) else {
            return self.release(out);
        };
        m.place(&mut item, target.rect);
        let masked = match redraw {
            true => {
                let masking = (d.mask, d.mask_texture.as_deref(), d.mask_strength);
                self.mask(&mut item, masking, m, size.unwrap_or(canvas), target)
            }
            false => Src::White,
        };
        let (mode, back) = match redraw {
            true => self.blend(&mut item, d.blend, bounds, target, false),
            false if d.opacity >= 1.0 => (Mode::Replace, Src::Clear),
            false => (Mode::Normal, Src::Clear),
        };
        let slot = self.pool.push(&item);
        let tex = [out, masked, back];
        self.ops.push(Op::Draw { target: target.src, entry: "fs_texture", mode, slot, tex });
        [out, masked, back].into_iter().for_each(|s| self.release(s));
    }
}
