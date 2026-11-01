use super::pipes::{Pipes, compiled, module};
use crate::Renderer;
use crate::texture::{GpuImage, create_texture, gpu_image};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{BindGroupLayout, RenderPipeline, TextureFormat, TextureUsages, TextureView};

const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const ENTRIES: [&str; 3] = ["fs_seed", "fs_jump", "fs_field"];

pub(crate) type Fielders = Arc<(BindGroupLayout, [RenderPipeline; 3])>;

fn layout(device: &wgpu::Device) -> BindGroupLayout {
    let visibility = wgpu::ShaderStages::FRAGMENT;
    let sample_type = wgpu::TextureSampleType::Float { filterable: false };
    let view_dimension = wgpu::TextureViewDimension::D2;
    let tex = wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false };
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let entries = [tex, tex, uniform].into_iter().enumerate().map(|(i, ty)| wgpu::BindGroupLayoutEntry {
        binding: i as u32,
        visibility,
        ty,
        count: None,
    });
    let entries: Vec<_> = entries.collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries })
}

impl Pipes {
    pub(crate) fn fielders(&self, device: &wgpu::Device) -> Fielders {
        let mut made = self.fields.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let made = made.get_or_insert_with(|| {
            compiled(3);
            let module = module(device, include_str!("field.wgsl").into());
            let group = layout(device);
            let bind_group_layouts = [Some(&group)];
            let shape = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &bind_group_layouts,
                immediate_size: 0,
            });
            let pipelines = ENTRIES.map(|entry| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("wprs-scene text field"),
                    layout: Some(&shape),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        targets: &[Some(FORMAT.into())],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
            });
            Arc::new((group, pipelines))
        });
        made.clone()
    }
}

fn steps(reach: u32) -> Vec<u32> {
    let first = (reach + 2).next_power_of_two();
    std::iter::successors(Some(first), |s| (*s > 1).then_some(s / 2)).chain([1]).collect()
}

impl Renderer {
    pub(crate) fn field(&self, cover: &GpuImage, reach: u32) -> GpuImage {
        let device = &self.device;
        let fielders = self.pipes().fielders(device);
        let (group, pipelines) = &*fielders;
        let usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
        let make = || create_texture(device, "wprs-scene text field", cover.size(), FORMAT, usage);
        let (seeds, out) = ([make(), make()], make());
        let views = seeds.each_ref().map(|t| t.create_view(&Default::default()));
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut pass = |pipeline: &RenderPipeline, to: &TextureView, from: &TextureView, step: u32| {
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[step as f32, 0.0, 0.0, 0.0]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let view = |v| wgpu::BindingResource::TextureView(v);
            let entries = [
                wgpu::BindGroupEntry { binding: 0, resource: view(&cover.view) },
                wgpu::BindGroupEntry { binding: 1, resource: view(from) },
                wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
            ];
            let bound = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: group,
                entries: &entries,
            });
            let load = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let ops = wgpu::Operations { load, store: wgpu::StoreOp::Store };
            let attachment =
                wgpu::RenderPassColorAttachment { view: to, depth_slice: None, resolve_target: None, ops };
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wprs-scene text field"),
                color_attachments: &[Some(attachment)],
                ..Default::default()
            });
            p.set_pipeline(pipeline);
            p.set_bind_group(0, &bound, &[]);
            p.draw(0..3, 0..1);
        };
        pass(&pipelines[0], &views[0], &views[1], 0);
        let mut at = 0;
        for step in steps(reach) {
            pass(&pipelines[1], &views[1 - at], &views[at], step);
            at = 1 - at;
        }
        let done = out.create_view(&Default::default());
        pass(&pipelines[2], &done, &views[at], 0);
        self.queue.submit([encoder.finish()]);
        gpu_image(out)
    }
}
