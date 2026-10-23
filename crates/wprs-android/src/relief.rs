use crate::depth::{self, Raise};
use crate::gpu::Gpu;
use crate::power::Power;
use crate::tilt::{self, Motion};
use crate::{choice, store};
use image::Luma;
use std::path::Path;
use std::time::Instant;
use wprs_core::{Fit, UvRect};
use wprs_render::{DepthMap, GpuImage, Layer, Relief};

const BATTERY_STEPS: u32 = 8;

pub(crate) struct Raised {
    key: (u64, String, Raise),
    depth: GpuImage,
    flat: bool,
    motion: Motion,
}

#[derive(Default)]
pub(crate) struct Tilted(u64, bool);

impl Tilted {
    pub(crate) fn rate(&mut self, preview: bool) -> u32 {
        let generation = store::generation();
        if self.0 != generation {
            let shown = choice::shown(preview).filter(|shown| !shown.video);
            *self = Tilted(generation, shown.is_some_and(|shown| depth::raised(&shown.path).is_some()));
        }
        let power = Power::now();
        if self.1 && power.moving() { power.fps(preview) } else { 0 }
    }
}

pub(crate) fn raise(old: &mut Option<Raised>, gpu: &Gpu, shown: Option<(&str, &Path)>) -> bool {
    let key = shown.and_then(|(uid, path)| Some((gpu.epoch, uid.to_owned(), depth::raised(path)?)));
    if old.as_ref().map(|raised| &raised.key) == key.as_ref() {
        return false;
    }
    let motion =
        old.take().filter(|raised| key.as_ref().is_some_and(|k| k.1 == raised.key.1)).map(|r| r.motion);
    *old = key.and_then(|key| {
        let map = depth::map(&key.2);
        let flat = map.is_none();
        let map = map.unwrap_or_else(|| DepthMap::from_pixel(1, 1, Luma([u16::MAX])));
        let depth = gpu.renderer.upload_depth(&map, None).inspect_err(|e| log::warn!("{e}")).ok()?;
        Some(Raised { key, depth, flat, motion: motion.unwrap_or_default() })
    });
    true
}

impl Raised {
    pub(crate) fn step(
        &mut self,
        offset: f32,
        clock: (i64, u32),
        moving: bool,
        size: (u32, u32),
    ) -> (bool, bool) {
        if !moving {
            return (self.motion.rest(), false);
        }
        let parallax = &self.key.2.parallax;
        let pixels = parallax.strength * size.0.max(size.1) as f32;
        self.motion.step(tilt::goal(offset, Instant::now()), parallax.smoothing_ms, clock, pixels)
    }

    fn relief(&self, aspect: f32, charging: bool) -> Relief<'_> {
        let p = &self.key.2.parallax;
        let steps = match () {
            _ if self.flat => 1,
            _ if charging => p.steps,
            _ => p.steps.min(BATTERY_STEPS),
        };
        let shift = tilt::shift(self.motion.at(), p.strength, aspect);
        Relief { depth: &self.depth, shift, focus: p.focus, steps, overscan: p.overscan }
    }
}

pub(crate) fn layer<'a>(
    image: Option<&'a GpuImage>,
    opacity: f32,
    look: (Fit, [u8; 3], f32),
    size: (u32, u32),
    raised: Option<(&'a Raised, bool)>,
) -> Layer<'a> {
    let (fit, background, shown) = look;
    let uv = image.map(|image| {
        let mut uv = UvRect::compute(fit, image.size(), size);
        if fit == Fit::Fill {
            uv.offset[0] = shown * (1.0 - uv.scale[0]);
        }
        uv
    });
    let aspect = image.map_or(1.0, |i| i.size().0 as f32 / i.size().1.max(1) as f32);
    let relief = raised.filter(|_| image.is_some()).map(|(raised, charging)| raised.relief(aspect, charging));
    Layer { image, fit, background, opacity, uv, relief }
}
