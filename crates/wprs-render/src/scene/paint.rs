use super::item::Rect;
use super::pool::{Pool, Tex};
use super::sampling::Sampling;
use super::state::Sim;
use super::{Cached, Image, Op, Src};
use crate::{GpuImage, Renderer};
use std::collections::HashMap;
use std::fmt::Write;
use std::hash::{DefaultHasher, Hasher};
use std::ops::Range;
use wprs_scene::{Affine, Frame};

mod geometry;
mod passes;

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
    pub(crate) rasters: &'a HashMap<u64, super::rasters::Raster>,
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
    pub(crate) flat: std::cell::Cell<Option<(wprs_scene::Lean, wprs_scene::Lean)>>,
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

    pub(crate) fn text(&self, i: usize) -> Option<(&'a GpuImage, [f32; 2], f32)> {
        let rasters = self.rasters;
        let at = self.texts.binary_search_by_key(&i, |(i, _)| *i).ok()?;
        rasters.get(&self.texts[at].1).map(|r| (&r.image, r.size, r.field))
    }

    pub(crate) fn picture(&self, id: &str) -> Option<&'a GpuImage> {
        let (videos, images) = (self.videos, self.images);
        let video = videos.iter().find(|(name, _)| *name == id).map(|(_, v)| *v);
        video.or_else(|| images.get(id).map(|image| &image.0))
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
            let looks = (d.tint, d.brightness, d.mask_strength, d.isolate, d.lean);
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
}
