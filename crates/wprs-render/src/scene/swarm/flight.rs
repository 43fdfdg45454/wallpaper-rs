use super::{B, Swarms, buffer, module};
use crate::Renderer;
use wgpu::{BindGroup, Buffer};
use wprs_scene::{Bake, ROPE_POINTS};

pub(super) const ROPE_BYTES: u64 = (4096 + ROPE_POINTS as u64 * 12) * 4;
pub(super) const ROPE_AT: u64 = 4096 * 4;

type Gpu = (Buffer, Buffer, BindGroup);

impl Swarms {
    pub(super) fn extras(&mut self, r: &Renderer, gpu: bool) {
        let bakes = || self.herds.iter().map(|h| &h.bake);
        let flocks = bakes().any(|b| b.grids.iter().any(|g| g[1] > 0));
        let ropes = bakes().any(|b| b.ropes.iter().any(Option::is_some));
        let pipe = |r: &Renderer, src: &str, layout: &wgpu::BindGroupLayout, entries: &[&str]| {
            let module = module(r, &[include_str!("kinds.wgsl"), src].join("\n"));
            let layouts = [Some(layout)];
            let desc = wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &layouts,
                immediate_size: 0,
            };
            let layout = r.device.create_pipeline_layout(&desc);
            entries
                .iter()
                .map(|entry| {
                    r.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(entry),
                        layout: Some(&layout),
                        module: &module,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        cache: None,
                    })
                })
                .collect::<Vec<_>>()
        };
        if gpu && flocks && self.flocking.is_none() {
            let names = ["flock_clear", "flock_count", "flock_scan", "flock_scatter", "flock_steer"];
            let made = pipe(r, include_str!("flock.wgsl"), &self.layouts[4], &names);
            self.flocking = made.try_into().ok();
        }
        if gpu && ropes && self.twining.is_none() {
            self.twining = pipe(r, include_str!("twine.wgsl"), &self.layouts[5], &["twine"]).pop();
        }
    }

    pub(super) fn strings(
        &self,
        r: &Renderer,
        (bake, p): (&Bake, usize),
        particles: &Buffer,
        gpu: Option<&Gpu>,
    ) -> (Option<Buffer>, Option<BindGroup>, Option<BindGroup>) {
        let usage =
            if gpu.is_some() { B::STORAGE | B::VERTEX | B::COPY_SRC } else { B::VERTEX | B::COPY_DST };
        let rope = bake.ropes[p].map(|_| buffer(r, ROPE_BYTES, usage));
        let Some((state, sims, _)) = gpu else { return (rope, None, None) };
        let size = wgpu::BufferSize::new(size_of::<wprs_scene::Sim>() as u64);
        let sim = || wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: sims, offset: 0, size });
        let group = |layout: &wgpu::BindGroupLayout, list: Vec<wgpu::BindingResource>| {
            let entries: Vec<_> = list
                .into_iter()
                .enumerate()
                .map(|(b, resource)| wgpu::BindGroupEntry { binding: b as u32, resource })
                .collect();
            r.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
        };
        let flock = (bake.grids[p][1] > 0).then(|| {
            group(&self.layouts[4], vec![particles.as_entire_binding(), state.as_entire_binding(), sim()])
        });
        let twine = rope.as_ref().map(|rope| {
            let list = vec![
                particles.as_entire_binding(),
                state.as_entire_binding(),
                rope.as_entire_binding(),
                sim(),
            ];
            group(&self.layouts[5], list)
        });
        (rope, flock, twine)
    }
}
