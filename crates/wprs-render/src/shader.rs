use crate::{Layer, Renderer, draw::TARGET_FORMAT};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wprs_core::{Fit, UvRect};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    offset: [f32; 2],
    scale: [f32; 2],
    background: [f32; 4],
    flags: [u32; 4],
}

pub(crate) fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("wprs-render bind group layout"),
        entries: &[entry(0, uniform), entry(1, texture), entry(2, sampler)],
    })
}

pub(crate) fn pipelines(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> [wgpu::RenderPipeline; 2] {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("wprs-render shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("wprs-render pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    let fade = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::OneMinusConstant,
        operation: wgpu::BlendOperation::Add,
    };
    [None, Some(wgpu::BlendState { color: fade, alpha: fade })].map(|blend| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wprs-render pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { blend, ..TARGET_FORMAT.into() })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    })
}

impl Renderer {
    pub(crate) fn bind_group(&self, layer: &Layer<'_>, target: (u32, u32)) -> wgpu::BindGroup {
        let uv = layer.image.map_or(UvRect::compute(Fit::Stretch, (1, 1), (1, 1)), |image| {
            UvRect::compute(layer.fit, image.size(), target)
        });
        let [r, g, b] = layer.background.map(srgb_to_linear);
        let params = Params {
            offset: uv.offset,
            scale: uv.scale,
            background: [r, g, b, 1.0],
            flags: [uv.repeat as u32, layer.image.is_some() as u32, 0, 0],
        };
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wprs-render params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view = &layer.image.unwrap_or(&self.placeholder).view;
        let sampler = if uv.repeat { &self.repeat } else { &self.clamp };
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        })
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
