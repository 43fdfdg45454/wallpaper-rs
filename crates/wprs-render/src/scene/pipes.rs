use super::chains::{Shelf, lock};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::{BindGroupLayout, BlendComponent as C, BlendFactor as F, BlendOperation, TextureFormat};
use wgpu::{ShaderModule, TextureViewDimension as Dim};
use wprs_scene::FRAME_WGSL;

pub(crate) const STRIDE: u64 = 512;

thread_local! {
    static COMPILED: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn compiled(more: u32) -> u32 {
    COMPILED.with(|n| {
        n.set(n.get().wrapping_add(more));
        n.get()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Mode {
    Normal,
    Add,
    Multiply,
    Screen,
    Replace,
    Atop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Shader {
    Scene(&'static str),
    Fx(u64, &'static str),
}

pub(crate) type Key = (Shader, Mode, TextureFormat);

pub(crate) struct Pipes {
    composite: ShaderModule,
    pub(crate) shelf: Arc<Mutex<Shelf>>,
    pub(crate) frame: BindGroupLayout,
    pub(crate) textures: BindGroupLayout,
    pub(crate) fx_frame: BindGroupLayout,
    pub(crate) fx: BindGroupLayout,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) wrap: wgpu::Sampler,
    pub(crate) near: wgpu::Sampler,
    pub(crate) shrinks: Mutex<HashMap<TextureFormat, wgpu::RenderPipeline>>,
}

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    let visibility = wgpu::ShaderStages::VERTEX_FRAGMENT;
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

fn uniform(dynamic: bool) -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: dynamic,
        min_binding_size: None,
    }
}

fn texture(view_dimension: Dim) -> wgpu::BindingType {
    let sample_type = wgpu::TextureSampleType::Float { filterable: true };
    wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false }
}

pub(crate) fn blend(mode: Mode) -> Option<wgpu::BlendState> {
    let c = |src_factor, dst_factor| C { src_factor, dst_factor, operation: BlendOperation::Add };
    let over = c(F::One, F::OneMinusSrcAlpha);
    let (color, alpha) = match mode {
        Mode::Replace => return None,
        Mode::Normal => (over, over),
        Mode::Add => (c(F::One, F::One), over),
        Mode::Multiply => (c(F::Dst, F::OneMinusSrcAlpha), over),
        Mode::Screen => (c(F::One, F::OneMinusSrc), over),
        Mode::Atop => (c(F::DstAlpha, F::OneMinusSrcAlpha), c(F::Zero, F::One)),
    };
    Some(wgpu::BlendState { color, alpha })
}

impl Pipes {
    pub(crate) fn new(device: &wgpu::Device) -> Pipes {
        let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wprs-scene layout"),
                entries,
            })
        };
        let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let flat = |b| entry(b, texture(Dim::D2));
        let fx: Vec<_> = std::iter::once(entry(0, uniform(true)))
            .chain((1..=6).map(flat))
            .chain([entry(7, texture(Dim::D3))])
            .collect();
        let sampler_of = |address_mode, filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wprs-scene sampler"),
                address_mode_u: address_mode,
                address_mode_v: address_mode,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            })
        };
        let (linear, clamp) = (wgpu::FilterMode::Linear, wgpu::AddressMode::ClampToEdge);
        Pipes {
            composite: module(device, format!("{FRAME_WGSL}\n{}", include_str!("scene.wgsl"))),
            shelf: Arc::default(),
            frame: layout(&[
                entry(0, uniform(false)),
                entry(1, sampler),
                entry(2, uniform(true)),
                entry(3, sampler),
            ]),
            textures: layout(&[flat(0), flat(1), flat(2)]),
            fx_frame: layout(&[entry(0, uniform(false)), entry(1, sampler), entry(2, sampler)]),
            fx: layout(&fx),
            sampler: sampler_of(clamp, linear),
            wrap: sampler_of(wgpu::AddressMode::Repeat, linear),
            near: sampler_of(clamp, wgpu::FilterMode::Nearest),
            shrinks: Mutex::default(),
        }
    }

    pub(crate) fn get(
        &self,
        device: &wgpu::Device,
        shader: Shader,
        mode: Mode,
        format: TextureFormat,
    ) -> wgpu::RenderPipeline {
        let mut guard = lock(&self.shelf);
        let shelf = &mut *guard;
        let key = (shader, mode, format);
        if let Some(found) = shelf.cache.get(&key) {
            return found.clone();
        }
        let (module, entry, vertex, groups, strip) = match shader {
            Shader::Scene(entry) => (&self.composite, entry, "vs_item", [&self.frame, &self.textures], true),
            Shader::Fx(id, entry) => (
                shelf.modules.get(&id).expect("created first"),
                entry,
                "vs_main",
                [&self.fx_frame, &self.fx],
                false,
            ),
        };
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wprs-scene pipeline layout"),
            bind_group_layouts: &[Some(groups[0]), Some(groups[1])],
            immediate_size: 0,
        });
        let topology = if strip {
            wgpu::PrimitiveTopology::TriangleStrip
        } else {
            wgpu::PrimitiveTopology::TriangleList
        };
        let target = wgpu::ColorTargetState { blend: blend(mode), ..format.into() };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vertex),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(target)],
            }),
            primitive: wgpu::PrimitiveState { topology, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        shelf.cache.insert(key, pipeline.clone());
        compiled(1);
        pipeline
    }
}

pub(crate) fn module(device: &wgpu::Device, source: String) -> ShaderModule {
    let source = wgpu::ShaderSource::Wgsl(source.into());
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("wprs-scene shader"), source })
}
