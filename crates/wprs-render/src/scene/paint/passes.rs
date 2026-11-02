use super::super::item::{Rect, invert};
use super::super::pool::BUDGET;
use super::super::{Op, Src};
use super::{Painter, Target};
use std::ops::Range;
use wprs_scene::{Blend, Content, Layer};

impl<'a> Painter<'a, '_, '_> {
    pub(crate) fn run(&mut self, canvas: Src, background: [f64; 4], keep: bool) {
        self.spend();
        let frame = self.frame;
        let draws = &frame.draws;
        let (mut static_end, mut i, mut fed) = (0, 0, 0u64);
        while !self.frame.keep && i < draws.len() && !draws[i].dynamic && !self.shifting(i..draws[i].end) {
            let (key, moving) = self.feeding(i..draws[i].end);
            if moving {
                break;
            }
            (i, fed) = (draws[i].end, fed.rotate_left(7) ^ key);
            static_end = i;
        }
        let root = Target { src: canvas, rect: self.full };
        let (origin, size) =
            self.clip.map_or(([0, 0], self.full.size()), |c| ([c.x as u32, c.y as u32], c.size()));
        if static_end == 0 {
            match self.clip {
                _ if keep => {}
                Some(_) => self.fill(canvas, background),
                None => self.ops.push(Op::Clear(canvas, background)),
            }
        } else {
            let key = self.digest(0..static_end, (self.full, background.map(f64::to_bits), fed));
            if self.caches.get(&0).is_none_or(|c| c.key != key) {
                self.cache(0, key, self.full.size());
                self.ops.push(Op::Clear(Src::Cache(0), background));
                self.list(0..static_end, Target { src: Src::Cache(0), rect: self.full }, false);
            } else {
                self.reused += 1;
            }
            self.kept.push(0);
            self.ops.push(Op::Copy { from: Src::Cache(0), to: canvas, origin, size, at: origin });
        }
        self.list(static_end..draws.len(), root, false);
        self.unshare();
    }

    pub(crate) fn list(&mut self, range: Range<usize>, target: Target, clip: bool) {
        let mut i = range.start;
        while i < range.end {
            let frame = self.frame;
            let d = &frame.draws[i];
            let atop = clip && i > range.start;
            match &d.content {
                _ if d.hidden => {}
                Content::Group { .. } if self.plain(i) => self.list(i + 1..d.end, target, false),
                _ if self.skip(i, target) => {}
                Content::Adjustment => self.adjust(i, target),
                Content::Group { .. } => self.apart(i, target, atop),
                _ if !d.effects.is_empty() || d.layer.cache || self.baking(i) => self.apart(i, target, atop),
                _ => self.direct(i, target, (d.opacity, d.blend, d.mask), atop),
            }
            i = d.end;
        }
    }

    pub(crate) fn apart(&mut self, i: usize, target: Target, atop: bool) {
        if self.reads_below(i) {
            return self.beneath(i, target, atop);
        }
        if let Some((src, rect)) = self.isolate(i) {
            self.composite(i, src, rect, target, atop);
            self.release(src);
        }
    }

    pub(crate) fn isolate(&mut self, i: usize) -> Option<(Src, Rect)> {
        let own = self.lean(i).and(self.frame.draws[i].lean).map(|l| (l, l.inverse()));
        let outer = own.map(|own| self.flat.replace(Some(own)));
        let out = self.isolated(i);
        outer.into_iter().for_each(|o| self.flat.set(o));
        out
    }

    fn isolated(&mut self, i: usize) -> Option<(Src, Rect)> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let rect = self.extent(i)?.pixels(self.full)?.bucket();
        let id = d.layer as *const Layer as usize;
        let (fed, moving) = self.feeding(i..d.end);
        let still = !d.dynamic && self.backdrop.is_none() && !moving && !self.shifting(i..d.end);
        let key = still.then(|| self.digest(i..d.end, (rect, self.srgb, fed)));
        if key.is_some() && self.caches.get(&id).map(|c| c.key) == key {
            self.reused += 1;
            self.kept.push(id);
            return Some((Src::Cache(id), rect));
        }
        let src = self.hold(i, rect.size())?;
        self.ops.push(Op::Clear(src, [0.0; 4]));
        let target = Target { src, rect };
        match d.content {
            Content::Group { clip } => self.list(i + 1..d.end, target, clip),
            _ => self.direct(i, target, (1.0, Blend::Normal, None), false),
        }
        let layer = self.geometry(i).map_or(rect.affine(), |(size, _)| self.local(i, size));
        let uv = invert(layer).then(rect.affine());
        let out = self.effects(i, src, rect, uv);
        if out != src {
            self.release(src);
        }
        let room = self.caches.values().map(|c| c.tex.bytes()).sum::<u64>() < BUDGET;
        let Some(key) = key.filter(|_| room) else { return Some((out, rect)) };
        let size = self.size_of(out);
        self.cache(id, key, size);
        self.kept.push(id);
        self.ops.push(Op::Copy { from: out, to: Src::Cache(id), origin: [0, 0], size, at: [0, 0] });
        self.release(out);
        Some((Src::Cache(id), rect))
    }

    pub(crate) fn size_of(&self, src: Src) -> (u32, u32) {
        match src {
            Src::Pool(i) => self.pool.busy[i].0.size(),
            Src::Ext(i) => self.exts[i].size(),
            Src::Cache(k) => self.caches.get(&k).map_or((1, 1), |c| c.tex.size()),
            Src::State(k, side) => self.states.get(&k).map_or((1, 1), |s| s.tex[side].size()),
            Src::White | Src::Clear | Src::Lut(_) | Src::Void => (1, 1),
            Src::Canvas => self.full.size(),
        }
    }
}
