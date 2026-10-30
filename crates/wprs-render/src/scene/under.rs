use super::item::{Item, Rect, invert, ndc};
use super::paint::{Painter, Target};
use super::pipes::Mode;
use super::{Op, Src};
use wprs_scene::Content;

impl Painter<'_, '_, '_> {
    pub(crate) fn reads_below(&self, i: usize) -> bool {
        self.frame.effects[self.frame.draws[i].effects.clone()].iter().any(|e| e.descriptor.below())
    }

    fn under(&mut self, rect: Rect, target: Target) -> Option<Src> {
        let src = self.acquire(rect.size())?;
        self.ops.push(Op::Clear(src, [0.0; 4]));
        if let Some(r) = rect.pixels(target.rect) {
            let origin = [(r.x - target.rect.x) as u32, (r.y - target.rect.y) as u32];
            let at = [(r.x - rect.x) as u32, (r.y - rect.y) as u32];
            self.ops.push(Op::Copy { from: target.src, to: src, origin, size: r.size(), at });
        }
        Some(src)
    }

    pub(crate) fn beneath(&mut self, i: usize, target: Target, atop: bool) {
        let frame = self.frame;
        let d = &frame.draws[i];
        let compose = frame.effects[d.effects.clone()].iter().any(|e| e.descriptor.compose);
        let Some(rect) = self.extent(i).and_then(|r| r.pixels(self.full)) else { return };
        let rect = if compose { rect } else { self.snug(rect) };
        let Some(back) = self.under(rect, target) else { return self.missed(i, None) };
        let outer = self.backdrop.replace(back);
        match compose.then(|| self.under(rect, target)).flatten() {
            Some(mixed) => {
                let over = Target { src: mixed, rect };
                match d.content {
                    Content::Group { clip } => self.list(i + 1..d.end, over, clip),
                    _ => self.direct(i, over, (d.opacity, d.blend, d.mask), false),
                }
                let layer = self.geometry(i).map_or(rect.affine(), |(size, _)| self.local(i, size));
                let out = self.effects(i, mixed, rect, invert(layer).then(rect.affine()));
                let mut item =
                    Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], ..Item::default() };
                (item.color, (item.m0, item.m1)) = ([1.0; 4], ndc(target.rect, rect.affine()));
                let slot = self.pool.push(&item);
                let tex = [out, Src::White, Src::Clear];
                self.ops.push(Op::Draw {
                    target: target.src,
                    entry: "fs_texture",
                    mode: Mode::Replace,
                    slot,
                    tex,
                });
                [out, mixed].into_iter().for_each(|s| self.release(s));
            }
            None => {
                if let Some((src, rect)) = self.isolate(i) {
                    self.composite(i, src, rect, target, atop);
                    self.release(src);
                }
            }
        }
        self.backdrop = outer;
        self.release(back);
    }
}
