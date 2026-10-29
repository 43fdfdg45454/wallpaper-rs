use super::{B, CHUNK, SIM, SceneView, Store, Swarms, buffer, module};
use crate::{RenderError, Renderer};
use wgpu::util::DeviceExt;
use wprs_scene::{Census, Cloud, Sim};

impl Swarms {
    pub(super) fn prepare(&mut self, r: &Renderer, gpu: bool) {
        self.gpu = gpu;
        if gpu && self.compute.is_none() {
            let module = module(r, concat!(include_str!("emit.wgsl"), "\n", include_str!("simulate.wgsl")));
            let layouts = [Some(&self.layouts[0])];
            let desc = wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &layouts,
                immediate_size: 0,
            };
            let layout = r.device.create_pipeline_layout(&desc);
            self.compute = Some(["simulate", "emit"].map(|entry| {
                r.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            }));
        }
        self.stores = (0..self.kits.len()).map(|i| Some(self.store(r, i))).collect();
        self.marks.resize(self.kits.len(), (0, 0));
    }

    pub(super) fn store(&self, r: &Renderer, i: usize) -> Store {
        let kit = &self.kits[i];
        let alloc = kit.bake.alloc;
        let extra = if self.gpu { B::STORAGE } else { B::empty() };
        let particles = buffer(r, u64::from(alloc) * 64, B::VERTEX | B::COPY_DST | B::COPY_SRC | extra);
        let init = |contents: &[u8], usage| {
            let desc =
                wgpu::util::BufferInitDescriptor { label: Some("wprs-scene particles"), contents, usage };
            r.device.create_buffer_init(&desc)
        };
        let gpu = self.gpu.then(|| {
            let free: Vec<u32> = (0..alloc).collect();
            let dead = init(bytemuck::cast_slice(&free), B::STORAGE | B::COPY_DST);
            let top = init(bytemuck::bytes_of(&(alloc as i32)), B::STORAGE | B::COPY_DST);
            let none = [[0.0f32; 2]];
            let points = if kit.bake.points.is_empty() { &none[..] } else { &kit.bake.points };
            let points = init(bytemuck::cast_slice(points), B::STORAGE);
            let sims = buffer(r, CHUNK as u64 * SIM, B::UNIFORM | B::COPY_DST);
            let size = wgpu::BufferSize::new(size_of::<Sim>() as u64);
            let sim = wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &sims, offset: 0, size });
            let whole = [&particles, &dead, &top, &points].map(|b| b.as_entire_binding());
            let resources = whole.into_iter().chain([sim]).enumerate();
            let entries: Vec<_> =
                resources.map(|(b, resource)| wgpu::BindGroupEntry { binding: b as u32, resource }).collect();
            let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.layouts[0], entries: &entries };
            let group = r.device.create_bind_group(&desc);
            (dead, top, sims, group)
        });
        let entries =
            [wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&kit.texture) }];
        let tex = r.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layouts[2],
            entries: &entries,
        });
        Store { particles, gpu, cloud: (!self.gpu).then(|| Cloud::new(alloc)), tex }
    }

    pub(crate) fn trim(&mut self) {
        self.stores.iter_mut().for_each(|s| *s = None);
    }

    pub(crate) fn slots(&self) -> u32 {
        self.stores.iter().zip(&self.kits).filter(|(s, _)| s.is_some()).map(|(_, k)| k.bake.alloc).sum()
    }
}

impl Renderer {
    pub fn census(&self, view: &SceneView) -> Result<Vec<(String, Census)>, RenderError> {
        let Some(swarms) = &view.swarms else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (kit, store) in swarms.kits.iter().zip(&swarms.stores) {
            let Some(store) = store else { continue };
            if let Some(cloud) = &store.cloud {
                out.push((kit.id.clone(), Census::of(&cloud.particles)));
                continue;
            }
            let size = store.particles.size();
            let staging = buffer(self, size, B::MAP_READ | B::COPY_DST);
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(&store.particles, 0, &staging, 0, size);
            let done = self.queue.submit([encoder.finish()]);
            self.map(&staging.slice(..), &Default::default(), done)?;
            let data =
                staging.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
            out.push((kit.id.clone(), Census::of(bytemuck::cast_slice(&data))));
        }
        Ok(out)
    }
}
