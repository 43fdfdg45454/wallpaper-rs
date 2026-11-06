use crate::RenderError;
use crate::shader::{bind_group_layout, module, pipeline};
use std::collections::HashMap;
use wgpu::{Features, TextureFormat};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Load {
    Clear,
    Keep,
    DontCare,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Variant {
    pub(super) name: &'static str,
    pub(super) format: TextureFormat,
    pub(super) load: Load,
    pub(super) store: bool,
    pub(super) rounds: u32,
    pub(super) half: bool,
    pub(super) fixed: bool,
    pub(super) blend: bool,
    pub(super) one_pass: bool,
    pub(super) split: bool,
    pub(super) stamps: bool,
    pub(super) scale: u32,
}

pub(super) struct Gpu {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) info: wgpu::AdapterInfo,
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
    pub(super) unused: wgpu::Buffer,
    pipes: HashMap<(TextureFormat, u32, bool, bool), wgpu::RenderPipeline>,
}

impl Gpu {
    pub(super) fn open(backends: wgpu::Backends) -> Result<Gpu, RenderError> {
        let instance = crate::adapter::instance(backends, None);
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        };
        let adapter = pollster::block_on(instance.request_adapter(&options))
            .map_err(|e| RenderError::NoAdapter(format!("{backends:?}: {e}")))?;
        let offered = adapter.limits();
        let required_limits = wgpu::Limits {
            max_texture_dimension_2d: offered.max_texture_dimension_2d,
            ..wgpu::Limits::downlevel_defaults()
        };
        let required_features = adapter.features() & (Features::TIMESTAMP_QUERY | Features::SHADER_F16);
        let descriptor = wgpu::DeviceDescriptor { required_limits, required_features, ..Default::default() };
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))
            .map_err(|e| RenderError::Device(e.to_string()))?;
        let layout = bind_group_layout(&device, 1);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let unused = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let (info, pipes) = (adapter.get_info(), HashMap::new());
        Ok(Gpu { device, queue, info, layout, sampler, unused, pipes })
    }

    pub(super) fn wait(&self) -> Result<(), RenderError> {
        let timeout = Some(std::time::Duration::from_secs(60));
        let wait = wgpu::PollType::Wait { submission_index: None, timeout };
        self.device.poll(wait).map(drop).map_err(|e| RenderError::DeviceLost(e.to_string()))
    }

    pub(super) fn has(&self, feature: Features) -> bool {
        self.device.features().contains(feature)
    }

    pub(super) fn pipe(&mut self, v: &Variant) -> wgpu::RenderPipeline {
        let key = (v.format, v.rounds, v.half, v.blend);
        let (device, layout) = (&self.device, &self.layout);
        let made = self.pipes.entry(key).or_insert_with(|| {
            let real = if v.half { "f16" } else { "f32" };
            let text = include_str!("../ceiling.wgsl")
                .replace("REAL", real)
                .replace("ROUNDS", &v.rounds.to_string());
            let text = if v.half { format!("enable f16;\n{text}") } else { text };
            let blend = v.blend.then_some(wgpu::BlendState::ALPHA_BLENDING);
            let target = wgpu::ColorTargetState { blend, ..v.format.into() };
            pipeline(device, layout, &module(device, &text), "fs_main", target)
        });
        made.clone()
    }
}
