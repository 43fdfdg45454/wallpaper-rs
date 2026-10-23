use super::item::{Rect, invert, scale_of, shape};
use super::pool::{BUDGET, Pool, Tex};
use super::state::Sim;
use super::{Cached, Image, Op, Src};
use crate::{GpuImage, Renderer};
use std::collections::HashMap;
use std::fmt::Write;
use std::hash::{DefaultHasher, Hasher};
use std::ops::Range;
use wprs_scene::{Affine, Blend, Content, Frame, Layer};

pub(crate) struct Painter<'a, 'p, 'f> {
    pub(crate) r: &'a Renderer,
    pub(crate) frame: &'a Frame<'f>,
    pub(crate) pool: &'p mut Pool,
    pub(crate) caches: &'p mut HashMap<usize, Cached>,
    pub(crate) states: &'p mut HashMap<usize, Sim>,
    pub(crate) images: &'a HashMap<String, Image>,
    pub(crate) luts: &'a [(String, Tex)],
    pub(crate) texts: &'a HashMap<usize, (&'a GpuImage, [f32; 2])>,
    pub(crate) videos: &'a [(&'a str, &'a GpuImage)],
    pub(crate) exts: Vec<&'a GpuImage>,
    pub(crate) screen: Affine,
    pub(crate) stage: Affine,
    pub(crate) full: Rect,
    pub(crate) srgb: bool,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) state_format: wgpu::TextureFormat,
    pub(crate) ops: Vec<Op>,
    pub(crate) reused: u32,
    pub(crate) kept: Vec<usize>,
    pub(crate) swarms: Option<&'p mut super::swarm::Swarms>,
}

#[derive(Clone, Copy)]
pub(crate) struct Target {
    pub(crate) src: Src,
    pub(crate) rect: Rect,
}

struct Hashing(DefaultHasher);

impl Write for Hashing {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

impl<'a> Painter<'a, '_, '_> {
    pub(crate) fn ext(&mut self, image: &'a GpuImage) -> Src {
        self.exts.push(image);
        Src::Ext(self.exts.len() - 1)
    }

    pub(crate) fn acquire(&mut self, size: (u32, u32)) -> Option<Src> {
        self.pool.acquire(&self.r.device, size, self.format).map(Src::Pool)
    }

    fn cache(&mut self, id: usize, key: u64, size: (u32, u32)) {
        let tex = match self.caches.remove(&id) {
            Some(c) if c.tex.size() == size => c.tex,
            _ => {
                self.pool.created += 1;
                Tex::new(&self.r.device, size, self.format)
            }
        };
        self.caches.insert(id, Cached { key, tex });
    }

    pub(crate) fn digest(&self, range: Range<usize>, extra: impl std::fmt::Debug) -> u64 {
        let mut h = Hashing(DefaultHasher::new());
        for d in &self.frame.draws[range] {
            let effects = &self.frame.effects[d.effects.clone()];
            let fields = (d.end, d.world, d.origin, d.opacity, d.blend, &d.content, d.mask, d.hidden);
            let _ = write!(h, "{:p}{fields:?}{:?}", d.layer, d.mask_texture);
            for e in effects {
                let _ = write!(
                    h,
                    "{}{:?}{}{:?}{:?}",
                    e.descriptor.id, e.slots, e.scale, e.mask_texture, e.textures
                );
            }
        }
        let _ = write!(h, "{extra:?}");
        h.0.finish()
    }

    pub(crate) fn geometry(&self, i: usize) -> Option<([f32; 2], [f32; 4])> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let unit = [0.0, 0.0, 1.0, 1.0];
        match &d.content {
            Content::Image { asset, size } | Content::Video { asset, size } => {
                let found = self.videos.iter().find(|(id, _)| id == asset);
                let natural = found.map(|(_, v)| [v.size().0 as f32, v.size().1 as f32]);
                let natural = natural.or_else(|| self.images.get(asset.as_ref()).map(|(_, size)| *size));
                Some((size.or(natural)?, unit))
            }
            Content::Text { .. } => Some((self.texts.get(&i)?.1, unit)),
            Content::Solid { size, .. } => Some((*size, unit)),
            Content::Spectrum { spectrum, .. } => Some((spectrum.size, unit)),
            Content::Particles { .. } => Some((self.swarms.as_ref()?.canvas, unit)),
            Content::Shape { shape: s, stroke, .. } => {
                let (size, ..) = shape(s);
                let px = 2.0 / scale_of(self.screen.then(d.world)).max(1e-6);
                let m = stroke.map_or(0.0, |s| s.1 / 2.0) + px;
                Some((size, [-m / size[0], -m / size[1], 1.0 + m / size[0], 1.0 + m / size[1]]))
            }
            _ => None,
        }
    }

    pub(crate) fn local(&self, i: usize, size: [f32; 2]) -> Affine {
        let frame = self.frame;
        let d = &frame.draws[i];
        let shift = Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]);
        self.screen.then(d.world).then(shift).then(Affine::scale(size))
    }

    fn reach(&self, i: usize) -> f32 {
        let frame = self.frame;
        let d = &frame.draws[i];
        self.frame.effects[d.effects.clone()].iter().map(|e| e.reach).sum::<f32>() * scale_of(self.screen)
    }

    pub(crate) fn extent(&self, i: usize) -> Option<Rect> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let rect = match &d.content {
            Content::Group { .. } => {
                let mut j = i + 1;
                let mut found: Option<Rect> = None;
                while j < d.end {
                    if let Some(r) = self.extent(j).filter(|_| !self.frame.draws[j].hidden) {
                        found = Some(found.map_or(r, |f| f.union(r)));
                    }
                    j = self.frame.draws[j].end;
                }
                found?
            }
            Content::Adjustment => self.full,
            _ => {
                let (size, [x0, y0, x1, y1]) = self.geometry(i)?;
                let m = self.local(i, size);
                Rect::around([[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| m.apply(p)))?
            }
        };
        Some(rect.grow(self.reach(i)))
    }

    pub(crate) fn run(&mut self, canvas: Src, background: [f64; 4]) {
        let draws = &self.frame.draws;
        let (mut static_end, mut i) = (0, 0);
        while i < draws.len() && !draws[i].dynamic {
            i = draws[i].end;
            static_end = i;
        }
        let root = Target { src: canvas, rect: self.full };
        if static_end == 0 {
            self.ops.push(Op::Clear(canvas, background));
        } else {
            let key = self.digest(0..static_end, (self.full, background.map(f64::to_bits)));
            if self.caches.get(&0).is_none_or(|c| c.key != key) {
                self.cache(0, key, self.full.size());
                self.ops.push(Op::Clear(Src::Cache(0), background));
                self.list(0..static_end, Target { src: Src::Cache(0), rect: self.full }, false);
            } else {
                self.reused += 1;
            }
            self.kept.push(0);
            self.ops.push(Op::Copy {
                from: Src::Cache(0),
                to: canvas,
                origin: [0, 0],
                size: self.full.size(),
                at: [0, 0],
            });
        }
        self.list(static_end..draws.len(), root, false);
    }

    fn list(&mut self, range: Range<usize>, target: Target, clip: bool) {
        let mut i = range.start;
        while i < range.end {
            let frame = self.frame;
            let d = &frame.draws[i];
            let atop = clip && i > range.start;
            match &d.content {
                _ if d.hidden => {}
                Content::Adjustment => self.adjust(i, target),
                Content::Group { clip: inner } => {
                    let plain = !inner && d.opacity >= 1.0 && d.blend == Blend::Normal && d.mask.is_none();
                    if plain && d.effects.is_empty() && !d.layer.cache {
                        self.list(i + 1..d.end, target, false);
                    } else if let Some((src, rect)) = self.isolate(i) {
                        self.composite(i, src, rect, target, atop);
                    }
                }
                _ if !d.effects.is_empty() || d.layer.cache => {
                    if let Some((src, rect)) = self.isolate(i) {
                        self.composite(i, src, rect, target, atop);
                    }
                }
                _ => self.direct(i, target, (d.opacity, d.blend, d.mask), atop),
            }
            i = d.end;
        }
    }

    pub(crate) fn isolate(&mut self, i: usize) -> Option<(Src, Rect)> {
        let frame = self.frame;
        let d = &frame.draws[i];
        let rect = self.extent(i)?.pixels(self.full)?.bucket();
        let id = d.layer as *const Layer as usize;
        let key = (!d.dynamic).then(|| self.digest(i..d.end, (rect, self.srgb)));
        if key.is_some() && self.caches.get(&id).map(|c| c.key) == key {
            self.reused += 1;
            self.kept.push(id);
            return Some((Src::Cache(id), rect));
        }
        let src = self.acquire(rect.size())?;
        self.ops.push(Op::Clear(src, [0.0; 4]));
        let target = Target { src, rect };
        match d.content {
            Content::Group { clip } => self.list(i + 1..d.end, target, clip),
            _ => self.direct(i, target, (1.0, Blend::Normal, None), false),
        }
        let layer = self.geometry(i).map_or(rect.affine(), |(size, _)| self.local(i, size));
        let uv = invert(layer).then(rect.affine());
        let out = self.effects(i, src, rect, uv);
        let room = self.caches.values().map(|c| c.tex.bytes()).sum::<u64>() < BUDGET;
        let Some(key) = key.filter(|_| room) else { return Some((out, rect)) };
        let size = self.size_of(out);
        self.cache(id, key, size);
        self.kept.push(id);
        self.ops.push(Op::Copy { from: out, to: Src::Cache(id), origin: [0, 0], size, at: [0, 0] });
        Some((Src::Cache(id), rect))
    }

    pub(crate) fn size_of(&self, src: Src) -> (u32, u32) {
        match src {
            Src::Pool(i) => self.pool.busy[i].size(),
            Src::Ext(i) => self.exts[i].size(),
            Src::Cache(k) => self.caches.get(&k).map_or((1, 1), |c| c.tex.size()),
            Src::State(k, side) => self.states.get(&k).map_or((1, 1), |s| s.tex[side].size()),
            Src::White | Src::Clear | Src::Lut(_) | Src::Void => (1, 1),
            Src::Canvas => self.full.size(),
        }
    }
}
