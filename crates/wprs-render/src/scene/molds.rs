use super::SceneView;
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use wgpu::{BufferUsages as B, TextureFormat as Format, TextureUsages as U};
use wprs_scene::{MESH_FLOATS, Runtime};

const FORMAT: Format = Format::Rgba8UnormSrgb;

pub(crate) struct Molder {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

pub(crate) struct Mold {
    generation: u64,
    pub(crate) image: GpuImage,
    group: wgpu::BindGroup,
    buffers: Option<(wgpu::Buffer, wgpu::Buffer)>,
}

fn molder(device: &wgpu::Device) -> Molder {
    let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
    let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
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
    let entries = [entry(0, both, uniform), entry(1, both, texture), entry(2, both, sampler)];
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("wprs-render molds"),
        entries: &entries,
    });
    let shader = crate::shader::module(device, include_str!("molds.wgsl"));
    let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
    let stride = (MESH_FLOATS * 4) as u64;
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    let pipelines = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("wprs-render molds"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wprs-render molds"),
        layout: Some(&pipelines),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_mold"),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_mold"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { blend, ..FORMAT.into() })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Molder { layout, pipeline }
}

impl Renderer {
    fn molder(&self) -> &Molder {
        self.molder.get_or_init(|| molder(&self.device))
    }

    fn fresh_mold(&self, view: &SceneView, size: (u32, u32), skin: Option<&str>) -> Mold {
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
        let image = gpu_image(create_texture(&self.device, "wprs-render mold", size, FORMAT, usage));
        let skin = skin.and_then(|id| view.images.get(id)).map_or(&view.pool.white.view, |i| &i.0.view);
        let values = [size.0 as f32, size.1 as f32, 0.0, 0.0];
        let uniform = crate::filled::filled(
            (&self.device, &self.queue),
            "wprs-render mold size",
            bytemuck::cast_slice(&values),
            B::UNIFORM,
        );
        let entries = [
            wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(skin) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.repeat) },
        ];
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render mold"),
            layout: &self.molder().layout,
            entries: &entries,
        });
        Mold { generation: u64::MAX, image, group, buffers: None }
    }

    pub(crate) fn molds(&self, view: &mut SceneView, runtime: &Runtime) -> Result<(), RenderError> {
        let scene = runtime.scene.clone();
        for (id, pool) in &scene.pools {
            let mesh = runtime.mesh(id);
            let generation = mesh.map_or(0, |m| m.generation);
            if view.molds.get(id).is_some_and(|m| m.generation == generation) {
                continue;
            }
            let [w, h] = pool.size.unwrap_or(scene.canvas.size).map(|v| v.clamp(1, self.max_edge()));
            if !view.molds.contains_key(id) {
                let made = self.scoped(|| self.fresh_mold(view, (w, h), pool.texture.as_deref()))?;
                view.molds.insert(id.clone(), made);
            }
            let mold = view.molds.get_mut(id).expect("made above");
            self.scoped(|| self.cast(mold, mesh))?;
            mold.generation = generation;
        }
        Ok(())
    }

    fn cast(&self, mold: &mut Mold, mesh: Option<&wprs_scene::Mesh>) {
        let filled = mesh.filter(|m| !m.indices.is_empty());
        if let Some(m) = filled {
            let fits = |b: &wgpu::Buffer, n: usize| b.size() >= n as u64;
            let (vb, ib) = (m.vertices.len() * 4, m.indices.len() * 4);
            if !mold.buffers.as_ref().is_some_and(|(v, i)| fits(v, vb) && fits(i, ib)) {
                let buffer = |size: usize, usage| {
                    self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("wprs-render mold"),
                        size: (size.next_power_of_two().max(256)) as u64,
                        usage: usage | B::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                mold.buffers = Some((buffer(vb, B::VERTEX), buffer(ib, B::INDEX)));
            }
            let (v, i) = mold.buffers.as_ref().expect("made above");
            self.queue.write_buffer(v, 0, bytemuck::cast_slice(&m.vertices));
            self.queue.write_buffer(i, 0, bytemuck::cast_slice(&m.indices));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let ops = wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Store,
        };
        let attachment = wgpu::RenderPassColorAttachment {
            view: &mold.image.view,
            depth_slice: None,
            resolve_target: None,
            ops,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render mold"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        if let (Some(m), Some((v, i))) = (filled, mold.buffers.as_ref()) {
            pass.set_pipeline(&self.molder().pipeline);
            pass.set_bind_group(0, &mold.group, &[]);
            pass.set_vertex_buffer(0, v.slice(..));
            pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.indices.len() as u32, 0, 0..1);
        }
        drop(pass);
        self.queue.submit([encoder.finish()]);
    }
}
