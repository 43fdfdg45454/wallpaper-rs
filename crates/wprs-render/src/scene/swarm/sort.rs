use super::{B, PARTICLE, Stacked, Store, Swarms, buffer, module};
use crate::Renderer;
use wgpu::Buffer;
use wprs_scene::{Cloud, Stack, stacked};

impl Swarms {
    pub(super) fn sorting(&self, r: &Renderer) -> [wgpu::ComputePipeline; 3] {
        let module = module(r, include_str!("sort.wgsl"));
        let layouts = [Some(&self.layouts[3])];
        let desc =
            wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 };
        let layout = r.device.create_pipeline_layout(&desc);
        ["tally", "scan", "scatter"].map(|entry| {
            r.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        })
    }

    pub(super) fn stacked(
        &self,
        r: &Renderer,
        (particles, alloc): (&Buffer, u32),
        order: Stack,
        size: f32,
    ) -> Stacked {
        let sorted = buffer(r, u64::from(alloc) * PARTICLE, B::VERTEX | B::STORAGE | B::COPY_DST);
        let gpu = self.gpu.then(|| {
            let bins = buffer(r, 1024, B::STORAGE | B::COPY_DST);
            let key = [order as u32, alloc, size.to_bits(), self.canvas[1].to_bits()];
            let key = crate::filled::filled(
                (&r.device, &r.queue),
                "wprs-scene particle order",
                bytemuck::cast_slice(&key),
                B::UNIFORM,
            );
            let parts = [particles, &sorted, &bins, &key];
            let entries: Vec<_> = parts
                .iter()
                .enumerate()
                .map(|(b, buffer)| wgpu::BindGroupEntry {
                    binding: b as u32,
                    resource: buffer.as_entire_binding(),
                })
                .collect();
            let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.layouts[3], entries: &entries };
            let group = r.device.create_bind_group(&desc);
            (bins, group)
        });
        Stacked { order, size, sorted, gpu, cpu: Vec::new() }
    }
}

pub(super) fn sort(
    r: &Renderer,
    (sorts, height): (Option<&[wgpu::ComputePipeline; 3]>, f32),
    stores: &mut [Store],
    clouds: &[Cloud],
    allocs: &[u32],
) {
    for (store, cloud) in stores.iter_mut().zip(clouds) {
        for st in &mut store.stacks {
            stacked(&cloud.particles, (st.order, st.size, height), &mut st.cpu);
            r.queue.write_buffer(&st.sorted, 0, bytemuck::cast_slice(&st.cpu));
        }
    }
    let Some(pipelines) = sorts.filter(|_| clouds.is_empty() && stores.iter().any(|s| !s.stacks.is_empty()))
    else {
        return;
    };
    let mut encoder = r.device.create_command_encoder(&Default::default());
    for (p, store) in stores.iter().enumerate() {
        for st in &store.stacks {
            let Some((bins, group)) = &st.gpu else { continue };
            encoder.clear_buffer(&st.sorted, 0, None);
            encoder.clear_buffer(bins, 0, None);
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, group, &[]);
            for (pipeline, groups) in
                pipelines.iter().zip([allocs[p].div_ceil(64), 1, allocs[p].div_ceil(64)])
            {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(groups, 1, 1);
            }
        }
    }
    r.queue.submit([encoder.finish()]);
}
