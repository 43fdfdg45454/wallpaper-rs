mod kit;
mod setup;
pub(crate) use setup::sprites;
mod sprite;
mod step;
mod store;

use super::SceneView;
use super::pipes::Mode;
use crate::Renderer;
use kit::{Kit, Look};
use std::collections::HashMap;
use wgpu::{BindGroup, BindGroupLayout, Buffer, BufferUsages as B, ShaderStages as S};
use wprs_scene::{Bake, Blend, Cloud, Quality};

const CHUNK: usize = 16;
const SIM: u64 = 2560;
const LOOK: u64 = 1536;
const PARTICLE: u64 = 80;

struct Store {
    particles: Buffer,
    gpu: Option<(Buffer, Buffer, BindGroup)>,
}

struct Flight {
    stores: Vec<Store>,
    clouds: Vec<Cloud>,
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
    trail: u32,
    kits: Vec<Kit>,
    herds: Vec<Herd>,
    pub(crate) sprites: Vec<Option<String>>,
    flights: Vec<Option<Flight>>,
    marks: Vec<(u64, u32)>,
    module: wgpu::ShaderModule,
    layouts: [BindGroupLayout; 3],
    compute: Option<[wgpu::ComputePipeline; 3]>,
    pipelines: HashMap<Mode, wgpu::RenderPipeline>,
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    looks: Vec<Look>,
    kitted: Vec<usize>,
    looked: Option<(Buffer, BindGroup)>,
    bytes: Vec<u8>,
    quality: Quality,
    none: Buffer,
}

fn entry(binding: u32, visibility: S, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

fn storage(read_only: bool) -> wgpu::BindingType {
    let ty = wgpu::BufferBindingType::Storage { read_only };
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }
}

fn uniform(size: usize) -> wgpu::BindingType {
    let (ty, min_binding_size) = (wgpu::BufferBindingType::Uniform, wgpu::BufferSize::new(size as u64));
    wgpu::BindingType::Buffer { ty, has_dynamic_offset: true, min_binding_size }
}

fn buffer(r: &Renderer, size: u64, usage: B) -> Buffer {
    let label = Some("wprs-scene particles");
    r.device.create_buffer(&wgpu::BufferDescriptor { label, size, usage, mapped_at_creation: false })
}

fn module(r: &Renderer, wgsl: &str) -> wgpu::ShaderModule {
    let source = wgpu::ShaderSource::Wgsl(wgsl.into());
    r.device
        .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("wprs-scene particles"), source })
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

    pub fn profile_particles(&mut self, on: bool) {
        self.swarms.iter_mut().for_each(|s| s.profile = on);
    }
}

impl Swarms {
    pub(crate) fn world(&self, flock: usize) -> bool {
        self.herds.get(flock).is_some_and(|h| h.world)
    }
}
