mod chains;
mod damage;
mod draw;
mod effects;
mod encode;
mod fonts;
mod item;
mod load;
mod mips;
mod paint;
mod pipes;
mod pool;
mod poster;
mod render;
mod sampling;
mod state;
#[cfg(test)]
mod steady;
mod swarm;
mod text;
mod uploads;
mod view;
mod warm;
mod woff2;

use crate::Renderer;
use crate::texture::GpuImage;
pub use fonts::{MAX_FONT, font};
pub(crate) use pipes::Pipes;
use pipes::{Mode, Shader};
use pool::Tex;
pub(crate) use sampling::layout;
use std::collections::HashMap;
use std::sync::Arc;
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
    Effect { target: Src, shader: Shader, slot: u32, tex: [Src; 7] },
    Particles { target: Src, system: usize, slot: u32, mode: Mode },
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
}

impl Renderer {
    pub fn software(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::Cpu
    }

    pub fn discrete(&self) -> bool {
        self.gpu.get_info().device_type == wgpu::DeviceType::DiscreteGpu
    }

    pub fn compute(&self) -> bool {
        swarm::compute(self)
    }

    pub fn tier(&self) -> wprs_scene::Tier {
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let total = meminfo
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:")?.trim().strip_suffix(" kB")?.parse().ok());
        wprs_scene::Tier::detect(&wprs_scene::Gpu {
            name: &self.gpu.get_info().name,
            software: self.software(),
            discrete: self.discrete(),
            compute: self.compute(),
            max_texture: self.device.limits().max_texture_dimension_2d,
            memory_mib: total.map_or(0, |kib: u64| kib / 1024),
        })
    }
}
