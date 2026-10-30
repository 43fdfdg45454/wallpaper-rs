use super::item::{Rect, invert, scale_of, shape};
use super::pool::{BUDGET, Pool, Tex};
use super::sampling::Sampling;
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
    pub(crate) samplings: &'a HashMap<String, Sampling>,
    pub(crate) luts: &'a [(String, Tex)],
    pub(crate) texts: &'a [(usize, u64)],
    pub(crate) rasters: &'a HashMap<u64, (GpuImage, [f32; 2], u64)>,
    pub(crate) videos: &'a [(&'a str, &'a GpuImage)],
    pub(crate) exts: &'p mut Vec<GpuImage>,
    pub(crate) screen: Affine,
    pub(crate) stage: Affine,
    pub(crate) full: Rect,
    pub(crate) clip: Option<Rect>,
    pub(crate) moment: u64,
    pub(crate) srgb: bool,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) state_format: wgpu::TextureFormat,
    pub(crate) ops: &'p mut Vec<Op>,
    pub(crate) reused: u32,
    pub(crate) kept: &'p mut Vec<usize>,
    pub(crate) outputs: &'p mut Vec<Src>,
    pub(crate) lease: &'p mut super::chains::Lease,
    pub(crate) refused: &'p mut std::collections::BTreeMap<String, String>,
    pub(crate) drawn: &'p mut Vec<u64>,
    pub(crate) sets: &'p mut Vec<[Src; 11]>,
    pub(crate) more: super::stops::More,
    pub(crate) made: &'p mut Vec<Src>,
    pub(crate) isolated: &'p mut Vec<(usize, Option<(Src, Rect)>)>,
    pub(crate) chains: &'p mut Vec<Option<(u64, bool)>>,
    pub(crate) paints: &'a [(String, String, Image)],
    pub(crate) backdrop: Option<Src>,
    pub(crate) swarms: Option<&'p mut super::swarm::Swarms>,
    pub(crate) flocks: &'a [u64],
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
    pub(crate) fn ext(&mut self, image: &GpuImage) -> Src {
        self.exts.push(image.clone());
        Src::Ext(self.exts.len() - 1)
    }

    pub(crate) fn text(&self, i: usize) -> Option<(&'a GpuImage, [f32; 2])> {
        let rasters = self.rasters;
        let at = self.texts.binary_search_by_key(&i, |(i, _)| *i).ok()?;
        rasters.get(&self.texts[at].1).map(|(image, size, _)| (image, *size))
    }

    pub(crate) fn acquire(&mut self, size: (u32, u32)) -> Option<Src> {
        self.borrow(size, self.format).map(Src::Pool)
    }

    pub(crate) fn release(&mut self, src: Src) {
        let shared = self.isolated.iter().any(|(_, found)| found.is_some_and(|(s, _)| s == src));
        if let Src::Pool(i) = src
            && !shared
        {
            self.pool.release(i);
        }
    }

    fn cache(&mut self, id: usize, key: u64, size: (u32, u32)) {
        let tex = match self.caches.remove(&id) {
            Some(c) if c.tex.size() == size => c.tex,
            old => {
                old.into_iter().for_each(|c| self.pool.give(c.tex));
                self.pool.take(&self.r.device, size, self.format)
            }
        };
        self.caches.insert(id, Cached { key, tex });
    }

    pub(crate) fn digest(&self, range: Range<usize>, extra: impl std::fmt::Debug) -> u64 {
        let mut h = Hashing(DefaultHasher::new());
        for d in &self.frame.draws[range] {
            let effects = &self.frame.effects[d.effects.clone()];
            let fields = (d.end, d.world, d.origin, d.opacity, d.blend, &d.content, d.mask, d.hidden);
            let looks = (d.tint, d.brightness, d.mask_strength, d.isolate);
            let _ = write!(h, "{:p}{fields:?}{looks:?}{:?}", d.layer, d.mask_texture);
            for e in effects {
                let (id, slots, scale, mask) =
                    (&e.descriptor.id, e.slots, e.scale, (e.mask, &e.mask_texture));
                let rest = (e.mask_strength, e.reach, e.chain, &e.textures, e.velocity, e.variant);
                let _ = write!(h, "{:p}{id}{slots:?}{scale}{mask:?}{rest:?}", e.effect);
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
            Content::Image { size, .. } | Content::Video { size, .. } => {
                Some((size.or(self.natural(i))?, unit))
            }
            Content::Text { .. } => Some((self.text(i)?.1, unit)),
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

    pub(crate) fn cell(&self, i: usize) -> ([f32; 4], Option<&'a Sampling>) {
        let frame = self.frame;
        let (asset, cell) = match &frame.draws[i].content {
            Content::Image { asset, frame, .. } => (asset, *frame),
            Content::Video { asset, .. } => (asset, None),
            _ => return ([0.0, 0.0, 1.0, 1.0], None),
        };
        let found = self.samplings.get(asset.as_ref());
        let rect = cell.and_then(|c| found?.cells.get(c as usize).copied());
        (rect.unwrap_or([0.0, 0.0, 1.0, 1.0]), found)
    }

    pub(crate) fn natural(&self, i: usize) -> Option<[f32; 2]> {
        let frame = self.frame;
        let (Content::Image { asset, .. } | Content::Video { asset, .. }) = &frame.draws[i].content else {
            return None;
        };
        let found = self.videos.iter().find(|(id, _)| id == asset);
        let natural = found.map(|(_, v)| [v.size().0 as f32, v.size().1 as f32]);
        let [w, h] = natural.or_else(|| self.images.get(asset.as_ref()).map(|image| image.1))?;
        let rect = self.cell(i).0;
        Some([w * rect[2], h * rect[3]])
    }

    pub(crate) fn local(&self, i: usize, size: [f32; 2]) -> Affine {
        let frame = self.frame;
        let d = &frame.draws[i];
        if let Content::Particles { flock, .. } = d.content
            && self.swarms.as_ref().is_some_and(|s| s.world(flock))
        {
            return self.screen.then(Affine::scale(size));
        }
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
        self.spend();
        let frame = self.frame;
        let draws = &frame.draws;
        let (mut static_end, mut i, mut fed) = (0, 0, 0u64);
        while i < draws.len() && !draws[i].dynamic {
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
                _ if !d.effects.is_empty() || d.layer.cache => self.apart(i, target, atop),
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
        let frame = self.frame;
        let d = &frame.draws[i];
        let rect = self.snug(self.extent(i)?.pixels(self.full)?);
        let id = d.layer as *const Layer as usize;
        let (fed, moving) = self.feeding(i..d.end);
        let still = !d.dynamic && self.backdrop.is_none() && !moving;
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

    pub(crate) fn snug(&self, rect: Rect) -> Rect {
        if rect == self.full { rect } else { rect.bucket() }
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
