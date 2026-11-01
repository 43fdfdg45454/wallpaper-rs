mod build;
mod builders;
mod capture;
mod chains;
mod clock;
mod damage;
mod draw;
mod effects;
mod encode;
mod fed;
mod field;
mod flight;
mod fonts;
mod held;
mod hits;
mod item;
mod limit;
mod load;
mod make;
mod map;
mod mips;
pub(crate) mod molds;
mod own;
mod packages;
mod paint;
mod pipes;
mod placed;
mod pool;
mod poster;
mod rasters;
mod ready;
mod render;
mod sampling;
mod sized;
pub(crate) mod skins;
mod sources;
mod state;
#[cfg(test)]
mod steady;
mod stops;
mod swarm;
mod text;
mod timed;
mod under;
mod uploads;
mod view;
mod warm;
mod woff2;

use crate::Renderer;
use crate::texture::GpuImage;
pub use builders::{finish_builds, stop_builds};
pub use clock::set_gpu_budget;
pub use flight::drawn_effects;
pub(crate) use flight::hanging;
pub use fonts::{MAX_FONT, font};
pub use packages::{MAX_SCENE_BUILD, set_build_timeout};
pub(crate) use pipes::Pipes;
use pipes::{Mode, Shader};
use pool::Tex;
pub(crate) use sampling::layout;
use std::collections::HashMap;
use std::sync::Arc;
pub use stops::{retry_effects, stop_effects};
pub use uploads::Pixels;
pub(crate) use uploads::Uploads;
pub use view::SceneView;
use wprs_scene::Lut;

const TEXT_FRAMES: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Src {
    Pool(usize),
    Ext(usize),
    Cache(usize),
    White,
    Clear,
    Canvas,
    State(usize, usize),
    Lut(usize),
    Void,
}

#[derive(Clone, Copy)]
pub(crate) enum Op {
    Clear(Src, [f64; 4]),
    Copy { from: Src, to: Src, origin: [u32; 2], size: (u32, u32), at: [u32; 2] },
    Draw { target: Src, entry: &'static str, mode: Mode, slot: u32, tex: [Src; 3] },
    Effect(Fill),
    Particles { target: Src, system: usize, slot: u32, mode: Mode },
    Read { from: Src, map: usize, size: (u32, u32) },
}

#[derive(Clone, Copy)]
pub(crate) struct Fill {
    pub(crate) target: Src,
    pub(crate) shader: Shader,
    pub(crate) slot: u32,
    pub(crate) tex: u32,
    pub(crate) more: Option<u32>,
    pub(crate) mode: Mode,
    pub(crate) load: bool,
    pub(crate) vertices: u32,
}

pub(crate) struct Cached {
    key: u64,
    tex: Tex,
}

#[derive(Default)]
pub(crate) struct Scratch {
    ops: Vec<Op>,
    exts: Vec<GpuImage>,
    kept: Vec<usize>,
    outputs: Vec<Src>,
    texts: Vec<(usize, u64)>,
    marks: Vec<damage::Mark>,
    drawn: Vec<u64>,
    sets: Vec<[Src; 11]>,
    made: Vec<Src>,
    isolated: Vec<(usize, Option<(Src, item::Rect)>)>,
    chains: Vec<Option<(u64, bool)>>,
    flocks: Vec<u64>,
}

pub(crate) type Image = Arc<(GpuImage, [f32; 2])>;

#[derive(Debug, Clone, Default)]
pub struct SceneAssets {
    pub images: HashMap<String, Arc<Pixels>>,
    pub fonts: HashMap<String, Arc<Vec<u8>>>,
    pub luts: HashMap<String, Arc<Lut>>,
    pub sheets: HashMap<String, Vec<f32>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SceneStats {
    pub passes: u32,
    pub draws: u32,
    pub created: u32,
    pub compiled: u32,
    pub textures: u32,
    pub reused: u32,
    pub steps: u32,
    pub particles: u32,
    pub sim_us: u32,
    pub damage: u32,
    pub in_flight: u32,
    pub gpu_us: u32,
    pub sim_gpu_us: u32,
}

impl Renderer {
    pub fn software(&self) -> bool {
        self.kind == wgpu::DeviceType::Cpu
    }

    pub fn discrete(&self) -> bool {
        self.kind == wgpu::DeviceType::DiscreteGpu
    }

    pub fn compute(&self) -> bool {
        swarm::compute(self)
    }

    pub(crate) fn gpu_class(&self) -> wprs_scene::Gpu<'_> {
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let total = meminfo
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:")?.trim().strip_suffix(" kB")?.parse().ok());
        wprs_scene::Gpu {
            name: &self.name,
            software: self.software(),
            discrete: self.discrete(),
            compute: self.compute(),
            max_texture: self.device.limits().max_texture_dimension_2d,
            memory_mib: total.map_or(0, |kib: u64| kib / 1024),
        }
    }
}
