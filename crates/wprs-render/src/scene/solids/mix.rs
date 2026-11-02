use crate::Renderer;
use crate::texture::GpuImage;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use wgpu::TextureFormat as Format;

const MIX: &str = "
@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_mix(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
@fragment
fn fs_mix(in: Out) -> @location(0) vec4<f32> {
    return textureSampleLevel(picture, smp, in.uv, 0.0);
}
";

pub(crate) struct Mixer {
    layout: wgpu::BindGroupLayout,
    module: wgpu::ShaderModule,
    pipes: Mutex<HashMap<Format, wgpu::RenderPipeline>>,
}

impl Renderer {
    fn mixer(&self) -> &Mixer {
        self.solids().mixer.get_or_init(|| {
            let entries = [
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ];
            let desc =
                wgpu::BindGroupLayoutDescriptor { label: Some("wprs-render solid mix"), entries: &entries };
            let layout = self.device.create_bind_group_layout(&desc);
            Mixer {
                layout,
                module: crate::shader::module(&self.device, MIX),
                pipes: Mutex::new(HashMap::new()),
            }
        })
    }

    fn mix_pipe(&self, format: Format) -> wgpu::RenderPipeline {
        let m = self.mixer();
        let mut pipes = m.pipes.lock().unwrap_or_else(PoisonError::into_inner);
        pipes
            .entry(format)
            .or_insert_with(|| {
                let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("wprs-render solid mix"),
                    bind_group_layouts: &[Some(&m.layout)],
                    immediate_size: 0,
                });
                let part = wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Constant,
                    dst_factor: wgpu::BlendFactor::OneMinusConstant,
                    operation: wgpu::BlendOperation::Add,
                };
                let blend = Some(wgpu::BlendState { color: part, alpha: part });
                self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("wprs-render solid mix"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &m.module,
                        entry_point: Some("vs_mix"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &m.module,
                        entry_point: Some("fs_mix"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState { blend, ..format.into() })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .clone()
    }

    pub(super) fn mix_group(&self, from: &GpuImage) -> wgpu::BindGroup {
        let entries = [
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&from.view) },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.solids().wrap),
            },
        ];
        let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.mixer().layout, entries: &entries };
        self.device.create_bind_group(&desc)
    }

    pub(super) fn mix_slices(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        group: &wgpu::BindGroup,
        into: &GpuImage,
        f: f32,
    ) {
        let pipe = self.mix_pipe(into.texture.format());
        let color = wgpu::RenderPassColorAttachment {
            view: &into.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render solid mix"),
            color_attachments: &[Some(color)],
            ..Default::default()
        });
        pass.set_pipeline(&pipe);
        pass.set_blend_constant(wgpu::Color {
            r: f64::from(f),
            g: f64::from(f),
            b: f64::from(f),
            a: f64::from(f),
        });
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}
