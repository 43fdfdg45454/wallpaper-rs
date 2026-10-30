use super::{B, LOOK, Look, PARTICLE, Swarms, buffer};
use crate::Renderer;
use crate::scene::pipes::{Mode, blend};

impl Swarms {
    pub(super) fn pipeline(&mut self, r: &Renderer, mode: Mode) {
        if self.pipelines.contains_key(&mode) {
            return;
        }
        let layouts = [Some(&self.layouts[1]), Some(&self.layouts[2])];
        let desc =
            wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 };
        let layout = r.device.create_pipeline_layout(&desc);
        let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4];
        let step_mode = wgpu::VertexStepMode::Instance;
        let buffers =
            [Some(wgpu::VertexBufferLayout { array_stride: PARTICLE, step_mode, attributes: &attributes })];
        let target = wgpu::ColorTargetState { blend: blend(mode), ..self.format.into() };
        let (module, compilation_options) = (&self.module, wgpu::PipelineCompilationOptions::default());
        let pipeline = r.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wprs-scene particles"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_particle"),
                compilation_options: compilation_options.clone(),
                buffers: &buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_particle"),
                compilation_options,
                targets: &[Some(target)],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(mode, pipeline);
        self.created += 1;
    }

    pub(crate) fn upload(&mut self, r: &Renderer) {
        let needed = self.looks.len() as u64 * LOOK;
        if needed == 0 {
            return;
        }
        if self.looked.as_ref().is_none_or(|(b, _)| b.size() < needed) {
            let buffer = buffer(r, needed.next_power_of_two().max(4 * LOOK), B::UNIFORM | B::COPY_DST);
            let size = wgpu::BufferSize::new(size_of::<Look>() as u64);
            let look =
                wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size });
            let entries = [
                wgpu::BindGroupEntry { binding: 0, resource: look },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ];
            let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.layouts[1], entries: &entries };
            let group = r.device.create_bind_group(&desc);
            self.looked = Some((buffer, group));
            self.created += 2;
        }
        self.bytes.clear();
        self.bytes.resize(needed as usize, 0);
        for (chunk, look) in self.bytes.chunks_mut(LOOK as usize).zip(&self.looks) {
            chunk[..size_of::<Look>()].copy_from_slice(bytemuck::bytes_of(look));
        }
        self.looked.iter().for_each(|(buffer, _)| r.queue.write_buffer(buffer, 0, &self.bytes));
    }

    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, flock: usize, slot: u32, mode: Mode) {
        let flight = self.flights.get(flock).and_then(Option::as_ref);
        let (Some(flight), Some((_, group)), Some(pipeline), Some(herd)) =
            (flight, &self.looked, self.pipelines.get(&mode), self.herds.get(flock))
        else {
            return;
        };
        pass.set_pipeline(pipeline);
        let first = (u64::from(slot) / LOOK) as usize;
        for (p, store) in flight.stores.iter().enumerate() {
            let Some(kit) = self.kitted.get(first + p).and_then(|k| self.kits.get(*k)) else { continue };
            pass.set_bind_group(0, group, &[slot + (p as u64 * LOOK) as u32]);
            pass.set_bind_group(1, &kit.tex, &[]);
            pass.set_vertex_buffer(0, store.particles.slice(..));
            pass.draw(0..4, 0..herd.bake.allocs[p]);
        }
    }
}
