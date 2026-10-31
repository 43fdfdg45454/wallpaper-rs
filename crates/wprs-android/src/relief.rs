use crate::depth::{self, Raise};
use crate::framing::placed;
use crate::gpu::Gpu;
use crate::power::Power;
use crate::store;
use crate::tilt::{self, Motion};
use image::Luma;
use std::path::Path;
use std::time::Instant;
use wprs_core::edit::{Edit, Geometry};
use wprs_core::{Fit, UvRect};
use wprs_render::{DepthMap, GpuImage, Layer, Relief};

const BATTERY_STEPS: u32 = 8;

pub(crate) struct Raised {
    key: (u64, String, Raise, Option<Geometry>),
    depth: GpuImage,
    flat: bool,
    motion: Motion,
}

#[derive(Default)]
pub(crate) struct Tilted(u64, bool, bool);

#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub(crate) fit: Fit,
    pub(crate) background: [u8; 3],
    pub(crate) pan: f32,
    pub(crate) crop: Option<[f32; 4]>,
}

impl Tilted {
    pub(crate) fn rate(&mut self, preview: bool, lock: bool) -> u32 {
        let generation = store::generation();
        if self.0 != generation {
            *self = Tilted(generation, tilt::wanted(preview, lock), tilt::scene(preview, lock));
        }
        let power = Power::now();
        tilt::rate(self.1 && !(self.2 && power.still), power, preview)
    }
}

pub(crate) fn raise(
    old: &mut Option<Raised>,
    gpu: &Gpu,
    shown: Option<(&str, &Path)>,
    geometry: Option<&Geometry>,
) -> bool {
    let key = shown
        .and_then(|(uid, path)| Some((gpu.epoch, uid.to_owned(), depth::raised(path)?, geometry.cloned())));
    if old.as_ref().map(|raised| &raised.key) == key.as_ref() {
        return false;
    }
    let motion =
        old.take().filter(|raised| key.as_ref().is_some_and(|k| k.1 == raised.key.1)).map(|r| r.motion);
    *old = key.and_then(|key| {
        let map = depth::maps::map(&key.2);
        let flat = map.is_none();
        let map = map.unwrap_or_else(|| DepthMap::from_pixel(1, 1, Luma([u16::MAX])));
        let turned = key.3.clone().map(|geometry| Edit { geometry, ..Edit::default() });
        let depth =
            gpu.renderer.upload_depth(&map, turned.as_ref()).inspect_err(|e| log::warn!("{e}")).ok()?;
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
        let goal = tilt::goal(offset, Instant::now());
        self.motion.step(goal, parallax.smoothing_ms, clock, tilt::pixels(parallax, size))
    }

    fn relief(&self, aspect: f32, visible: f32, charging: bool) -> (Relief<'_>, f32) {
        let p = &self.key.2.parallax;
        let steps = match () {
            _ if self.flat => 1,
            _ if charging => p.steps,
            _ => p.steps.min(BATTERY_STEPS),
        };
        let shift = tilt::shift(self.motion.at(), p.strength, aspect, visible);
        let relief = Relief { depth: &self.depth, shift, focus: p.focus, steps, overscan: 1.0 };
        (relief, tilt::overscan(p, aspect, visible))
    }
}

fn zoomed(uv: UvRect, zoom: f32) -> UvRect {
    let offset = [0, 1].map(|i| uv.offset[i] + uv.scale[i] * (1.0 - 1.0 / zoom) / 2.0);
    UvRect { offset, scale: uv.scale.map(|s| s / zoom), repeat: false }
}

pub(crate) fn layer<'a>(
    image: Option<&'a GpuImage>,
    opacity: f32,
    look: &Look,
    size: (u32, u32),
    raised: Option<(&'a Raised, bool)>,
) -> Layer<'a> {
    let uv = image.map(|image| placed(look.fit, look.crop, image.size(), size, look.pan));
    let aspect = image.map_or(1.0, |i| i.size().0 as f32 / i.size().1.max(1) as f32);
    let relief = raised.zip(uv).filter(|(_, uv)| !uv.repeat);
    let relief = relief.map(|((raised, charging), uv)| raised.relief(aspect, uv.scale[0], charging));
    let uv = uv.map(|uv| relief.as_ref().map_or(uv, |(_, zoom)| zoomed(uv, *zoom)));
    let (fit, background) = (look.fit, look.background);
    Layer { image, fit, background, opacity, uv, relief: relief.map(|(relief, _)| relief) }
}
