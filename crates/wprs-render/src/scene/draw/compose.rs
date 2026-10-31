use super::super::item::{Item, Rect, invert, ndc, toned};
use super::super::paint::{Painter, Target};
use super::super::pipes::Mode;
use super::super::{Op, Src};
use wprs_scene::Content;

impl Painter<'_, '_, '_> {
    pub(crate) fn composite(&mut self, i: usize, src: Src, rect: Rect, target: Target, atop: bool) {
        let frame = self.frame;
        let d = &frame.draws[i];
        let Some(bounds) = rect.pixels(target.rect) else { return };
        let group = matches!(d.content, Content::Group { .. });
        let color = if group { toned(d.tint, d.brightness, self.srgb, d.opacity) } else { [d.opacity; 4] };
        let mut item =
            Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], color, ..Item::default() };
        (item.m0, item.m1) = ndc(target.rect, rect.affine());
        let (m, size) = self
            .geometry(i)
            .map_or((rect.affine(), [rect.w, rect.h]), |(size, _)| (self.local(i, size), size));
        let masked =
            self.mask(&mut item, (d.mask, d.mask_texture.as_deref(), d.mask_strength), m, size, target);
        let (mode, back) = self.blend(&mut item, d.blend, bounds, target, atop);
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw {
            target: target.src,
            entry: "fs_texture",
            mode,
            slot,
            tex: [src, masked, back],
        });
        self.release(masked);
        self.release(back);
    }

    pub(crate) fn adjust(&mut self, i: usize, target: Target) {
        let frame = self.frame;
        let d = &frame.draws[i];
        if d.effects.is_empty() || self.idle(i) {
            return;
        }
        let out = self.effects(i, target.src, target.rect, invert(self.stage).then(target.rect.affine()));
        if out == target.src {
            return;
        }
        let mut item = Item {
            quad: [0.0, 0.0, 1.0, 1.0],
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [d.opacity; 4],
            ..Item::default()
        };
        (item.m0, item.m1) = ndc(target.rect, target.rect.affine());
        let mode = if d.opacity >= 1.0 { Mode::Replace } else { Mode::Normal };
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw {
            target: target.src,
            entry: "fs_texture",
            mode,
            slot,
            tex: [out, Src::White, Src::Clear],
        });
        self.release(out);
    }
}
