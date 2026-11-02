mod bent;
mod binds;
mod census;
mod chart;
mod dispatch;
mod fence;
mod flight;
mod kit;
mod lamp;
mod looks;
mod maps;
mod paint;
mod setup;
mod show;
pub(crate) use lamp::Lamp;
pub(crate) use setup::sprites;
mod sort;
mod sprite;
mod stage;
mod staging;
mod step;
mod store;
mod strands;
mod texels;

use super::SceneView;
use super::pipes::Mode;
use crate::Renderer;
use binds::{buffer, module};
use kit::{Kit, Look};
use std::collections::HashMap;
use wgpu::{BindGroup, BindGroupLayout, Buffer, BufferUsages as B};
use wprs_scene::{Bake, Blend, Cloud, Quality, Stack};

const CHUNK: usize = 16;
const SIM: u64 = 3584;
const LOOK: u64 = 1536;
const PARTICLE: u64 = 96;

type Stamp = (Option<wgpu::QuerySet>, bool);

struct Store {
    particles: Buffer,
    gpu: Option<(Buffer, Buffer, BindGroup)>,
    trail: Option<Buffer>,
    stacks: Vec<Stacked>,
    rope: Option<Buffer>,
    flock: Option<BindGroup>,
    twine: Option<BindGroup>,
}

struct Stacked {
    order: Stack,
    size: f32,
    sorted: Buffer,
    gpu: Option<(Buffer, BindGroup)>,
    cpu: Vec<wprs_scene::Particle>,
}

struct Flight {
    points: Buffer,
    stores: Vec<Store>,
    clouds: Vec<Cloud>,
    sims: Vec<wprs_scene::Sim>,
}

struct Herd {
    bake: Bake,
    world: bool,
}

pub(crate) struct Swarms {
    pub(crate) canvas: [f32; 2],
    pub(crate) steps: u32,
    pub(crate) micros: u32,
    pub(crate) created: u32,
    gpu: bool,
    profile: bool,
    kits: Vec<Kit>,
    herds: Vec<Herd>,
    pub(crate) sprites: Vec<Option<String>>,
    flights: Vec<Option<Flight>>,
    marks: Vec<(u64, u32)>,
    module: wgpu::ShaderModule,
    layouts: [BindGroupLayout; 7],
    compute: Option<[wgpu::ComputePipeline; 3]>,
    flocking: Option<[wgpu::ComputePipeline; 5]>,
    twining: Option<wgpu::ComputePipeline>,
    sorts: Option<[wgpu::ComputePipeline; 3]>,
    pipelines: HashMap<Mode, wgpu::RenderPipeline>,
    lits: HashMap<Mode, wgpu::RenderPipeline>,
    bend: Option<wgpu::RenderPipeline>,
    ribbons: HashMap<Mode, wgpu::RenderPipeline>,
    cords: HashMap<Mode, [wgpu::RenderPipeline; 2]>,
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    looks: Vec<Look>,
    kitted: Vec<usize>,
    looked: Option<(Buffer, BindGroup)>,
    bytes: Vec<u8>,
    quality: Quality,
    none: Buffer,
    stamp: Stamp,
    maps: Vec<maps::Map>,
    stages: HashMap<(Mode, u64), wgpu::RenderPipeline>,
    framing: BindGroupLayout,
    framed: Option<BindGroup>,
}

pub(super) fn compute(r: &Renderer) -> bool {
    let flags = r.gpu.get_downlevel_capabilities().flags;
    let limits = r.device.limits();
    flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        && limits.max_storage_buffers_per_shader_stage >= 4
        && limits.max_uniform_buffer_binding_size >= SIM
}

fn mode(blend: Blend, atop: bool) -> Mode {
    match blend {
        Blend::Normal if atop => Mode::Atop,
        Blend::Add => Mode::Add,
        Blend::Multiply => Mode::Multiply,
        Blend::Screen => Mode::Screen,
        _ => Mode::Normal,
    }
}

impl SceneView {
    pub fn gpu_particles(&self) -> Option<bool> {
        self.swarms.as_ref().map(|s| s.gpu)
    }

    pub fn set_gpu_particles(&mut self, r: &Renderer, gpu: bool) -> bool {
        let Some(swarms) = self.swarms.as_mut().filter(|_| compute(r) || !gpu) else { return false };
        swarms.prepare(r, gpu);
        true
    }

    pub fn set_particle_trails(&mut self, r: &Renderer, tier: wprs_scene::Tier) {
        if let Some(swarms) = self.swarms.as_mut() {
            swarms.herds.iter_mut().for_each(|h| h.bake.retrail(tier));
            swarms.prepare(r, swarms.gpu);
        }
    }

    pub fn profile_particles(&mut self, on: bool) {
        self.swarms.iter_mut().for_each(|s| s.profile = on);
    }
}

impl Swarms {
    pub(crate) fn world(&self, flock: usize) -> bool {
        self.herds.get(flock).is_some_and(|h| h.world)
    }
}
