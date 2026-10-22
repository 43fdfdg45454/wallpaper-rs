use std::collections::HashMap;
use std::sync::Mutex;
use wgpu::{BindGroupLayout, BlendComponent as C, BlendFactor as F, BlendOperation, TextureFormat};
use wprs_scene::{EFFECTS_WGSL, FRAME_WGSL, PRELUDE};

pub(crate) const STRIDE: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Normal,
    Add,
    Multiply,
    Screen,
    Replace,
    Atop,
}

type Key = (&'static str, Mode, TextureFormat);

pub(crate) struct Pipes {
    composite: wgpu::ShaderModule,
    effects: wgpu::ShaderModule,
    pub(crate) frame: BindGroupLayout,
    pub(crate) textures: BindGroupLayout,
    pub(crate) fx_frame: BindGroupLayout,
    pub(crate) fx: BindGroupLayout,
    pub(crate) sampler: wgpu::Sampler,
    cache: Mutex<HashMap<Key, wgpu::RenderPipeline>>,
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

fn texture() -> wgpu::BindingType {
    let sample_type = wgpu::TextureSampleType::Float { filterable: true };
    wgpu::BindingType::Texture {
        sample_type,
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    }
}

fn blend(mode: Mode) -> Option<wgpu::BlendState> {
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
        let fx: Vec<_> =
            std::iter::once(entry(0, uniform(true))).chain((1..=6).map(|b| entry(b, texture()))).collect();
        let module = |source: String| {
            let source = wgpu::ShaderSource::Wgsl(source.into());
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("wprs-scene shader"),
                source,
            })
        };
        Pipes {
            composite: module(format!("{FRAME_WGSL}\n{}", include_str!("scene.wgsl"))),
            effects: module(format!("{PRELUDE}\n{EFFECTS_WGSL}")),
            frame: layout(&[entry(0, uniform(false)), entry(1, sampler), entry(2, uniform(true))]),
            textures: layout(&[entry(0, texture()), entry(1, texture()), entry(2, texture())]),
            fx_frame: layout(&[entry(0, uniform(false)), entry(1, sampler)]),
            fx: layout(&fx),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wprs-scene sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            cache: Mutex::default(),
        }
    }

    pub(crate) fn get(
        &self,
        device: &wgpu::Device,
        entry: &'static str,
        mode: Mode,
        format: TextureFormat,
    ) -> wgpu::RenderPipeline {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let key = (entry, mode, format);
        if let Some(found) = cache.get(&key) {
            return found.clone();
        }
        let composite = ["fs_texture", "fs_solid", "fs_shape", "fs_spectrum"].contains(&entry);
        let (module, vertex, groups, strip) = match composite {
            true => (&self.composite, "vs_item", [&self.frame, &self.textures], true),
            false => (&self.effects, "vs_main", [&self.fx_frame, &self.fx], false),
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
        cache.insert(key, pipeline.clone());
        pipeline
    }
}
