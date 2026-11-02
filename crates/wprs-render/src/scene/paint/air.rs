use super::super::item::{Item, Rect, ndc};
use super::super::pipes::Mode;
use super::super::{Op, Src};
use super::{Painter, Target};
use std::hash::{DefaultHasher, Hash, Hasher};
use wprs_scene::Content;

const AIR: usize = 1;

fn quad() -> Item {
    Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4], ..Item::default() }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn slot(&self, k: usize) -> usize {
        self.frame.lamps.get(k).map_or(usize::MAX, |l| l.slot)
    }

    pub(crate) fn restless(&self, k: usize) -> bool {
        let draws = &self.frame.draws[..self.slot(k).min(self.frame.draws.len())];
        let video = |d: &wprs_scene::Draw<'_>| matches!(d.content, Content::Video { .. });
        let moving = draws.iter().any(|d| !d.hidden && (d.dynamic || video(d)));
        moving || ((self.pool.glow.moved || self.pool.glow.live) && draws.iter().any(|d| d.lit.is_some()))
    }

    fn airs(&self) -> bool {
        self.frame.quality.air(self.r.tier()).is_some() && self.pool.glow.airy()
    }

    pub(crate) fn first_air(&self) -> usize {
        let glow = &self.pool.glow;
        let airy = (0..glow.count()).filter(|&k| glow.now.bulbs[k].air[0] > 0.0);
        if self.airs() { airy.map(|k| self.slot(k)).min().unwrap_or(usize::MAX) } else { usize::MAX }
    }

    pub(crate) fn air_until(&mut self, at: usize, root: Target) {
        if !self.airs() {
            return;
        }
        while self.aired.0 < self.pool.glow.count() && self.slot(self.aired.0) <= at {
            self.aired.0 += 1;
            self.air(self.aired.0 - 1, root);
        }
    }

    fn air(&mut self, i: usize, root: Target) {
        let Some((step, share)) = self.frame.quality.air(self.r.tier()) else { return };
        let glow = &self.pool.glow;
        let (b, (layer, _, rect), seen) = (glow.now.bulbs[i], glow.marks[i], glow.seen);
        let Some(rect) = rect.pixels(self.full).filter(|_| b.air[0] > 0.0) else { return };
        let size = ((rect.w / step).ceil().max(1.0) as u32, (rect.h / step).ceil().max(1.0) as u32);
        let shafts = b.air[3] > 0.0;
        let behind = match () {
            _ if !shafts => Some(0),
            _ if self.restless(i) => None,
            _ => Some(self.digest(0..self.slot(i).min(self.frame.draws.len()), self.frame.background)),
        };
        let live = (self.pool.glow.live && b.air[2] > 0.0) || behind.is_none();
        let mut h = DefaultHasher::new();
        (bytemuck::bytes_of(&b), step.to_bits(), share.to_bits(), seen, size, behind).hash(&mut h);
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
        let Some(out) = out else { return };
        if reuse {
            self.reused += 1;
        } else {
            let back = if shafts { self.under(rect, root) } else { None };
            self.breathe(out, (i, rect, size), (step, share), back);
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

    fn breathe(
        &mut self,
        out: Src,
        (i, rect, size): (usize, Rect, (u32, u32)),
        k: (f32, f32),
        back: Option<Src>,
    ) {
        let mut item = quad();
        let whole = Rect { x: 0.0, y: 0.0, w: size.0 as f32, h: size.1 as f32 };
        (item.m0, item.m1) = ndc(whole, whole.affine());
        (item.lit_at, item.lit) = ([rect.x, rect.y, k.0, i as f32], [k.1, rect.w, rect.h, 0.0]);
        let mut maps = [Src::White; 8];
        maps[4..].copy_from_slice(&self.gobos());
        let (slot, tex) = (self.pool.push(&item), [back.unwrap_or(Src::White), Src::White, Src::Clear]);
        self.ops.push(Op::Lit { target: out, entry: "fs_air", mode: Mode::Replace, slot, tex, maps });
        back.into_iter().for_each(|s| self.release(s));
    }
}
