use super::super::item::{Item, Rect, ndc};
use super::super::pipes::Mode;
use super::super::{Op, Src};
use super::{Painter, Target};
use std::hash::{DefaultHasher, Hash, Hasher};

const AIR: usize = 1;

fn quad() -> Item {
    Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], ..Item::default() }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn air(&mut self, root: Target) {
        let Some((step, share)) = self.frame.quality.air(self.r.tier()) else { return };
        if !self.pool.glow.airy() {
            return;
        }
        for i in 0..self.pool.glow.count() {
            let glow = &self.pool.glow;
            let (b, (layer, _, rect)) = (glow.now.bulbs[i], glow.marks[i]);
            let Some(rect) = rect.pixels(self.full).filter(|_| b.air[0] > 0.0) else { continue };
            let size = ((rect.w / step).ceil().max(1.0) as u32, (rect.h / step).ceil().max(1.0) as u32);
            let live = glow.live && b.air[2] > 0.0;
            let mut h = DefaultHasher::new();
            (bytemuck::bytes_of(&b), step.to_bits(), share.to_bits(), glow.seen, size).hash(&mut h);
            let (id, key) = (layer + AIR, h.finish());
            let reuse = !live && self.caches.get(&id).is_some_and(|c| c.key == key);
            let out = match () {
                _ if reuse => Some(Src::Cache(id)),
                _ if live => self.acquire(size),
                _ => {
                    self.cache(id, key, size);
                    Some(Src::Cache(id))
                }
            };
            let Some(out) = out else { continue };
            if reuse {
                self.reused += 1;
            } else {
                let mut item = quad();
                let whole = Rect { x: 0.0, y: 0.0, w: size.0 as f32, h: size.1 as f32 };
                (item.m0, item.m1) = ndc(whole, whole.affine());
                (item.lit_at, item.lit) = ([rect.x, rect.y, step, i as f32], [share, 0.0, 0.0, 0.0]);
                let mut maps = [Src::White; 8];
                maps[4..].copy_from_slice(&self.gobos());
                let (slot, tex) = (self.pool.push(&item), [Src::White, Src::White, Src::Clear]);
                self.ops.push(Op::Lit { target: out, entry: "fs_air", mode: Mode::Replace, slot, tex, maps });
            }
            if !live {
                self.kept.push(id);
            }
            let mut item = quad();
            (item.m0, item.m1) = ndc(root.rect, rect.affine());
            let slot = self.pool.push(&item);
            let tex = [out, Src::White, Src::Clear];
            self.ops.push(Op::Draw { target: root.src, entry: "fs_texture", mode: Mode::Add, slot, tex });
            if live {
                self.release(out);
            }
        }
    }
}
