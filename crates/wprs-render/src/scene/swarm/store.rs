use super::{B, CHUNK, Flight, PARTICLE, SIM, SceneView, Store, Swarms, buffer, module};
use crate::{RenderError, Renderer};
use wprs_scene::Stack;
use wprs_scene::{Census, Cloud, Sim};

pub(super) fn fresh(alloc: u32, size: u32) -> Vec<u32> {
    let mut state = vec![0u32; size as usize];
    state[0] = alloc;
    state[4..4 + alloc as usize].iter_mut().enumerate().for_each(|(i, v)| *v = i as u32);
    state
}

impl Swarms {
    pub(super) fn prepare(&mut self, r: &Renderer, gpu: bool) {
        self.gpu = gpu;
        if gpu && self.compute.is_none() {
            let parts = [include_str!("sim.wgsl"), include_str!("shape.wgsl"), include_str!("emit.wgsl")];
            let rest = [include_str!("simulate.wgsl"), include_str!("step.wgsl")];
            let module = module(r, &parts.into_iter().chain(rest).collect::<Vec<_>>().join("\n"));
            let layouts = [Some(&self.layouts[0])];
            let desc = wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &layouts,
                immediate_size: 0,
            };
            let layout = r.device.create_pipeline_layout(&desc);
            self.compute = Some(["simulate", "emit", "settle"].map(|entry| {
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
        if gpu && self.sorts.is_none() && self.kits.iter().any(|k| k.order.is_some()) {
            self.sorts = Some(self.sorting(r));
        }
        self.flights = (0..self.herds.len()).map(|i| Some(self.flight(r, i))).collect();
        self.marks.resize(self.herds.len(), (0, 0));
    }

    pub(super) fn flight(&self, r: &Renderer, i: usize) -> Flight {
        let bake = &self.herds[i].bake;
        let init = |contents: &[u8], usage| {
            crate::filled::filled((&r.device, &r.queue), "wprs-scene particles", contents, usage)
        };
        let none = [[0.0f32; 2]];
        let points = if bake.points.is_empty() { &none[..] } else { &bake.points };
        let points = init(bytemuck::cast_slice(points), B::STORAGE);
        let mut stores: Vec<Store> = Vec::with_capacity(bake.allocs.len());
        for (p, alloc) in bake.allocs.iter().enumerate() {
            let extra = if self.gpu { B::STORAGE } else { B::empty() };
            let particles =
                buffer(r, u64::from(*alloc) * PARTICLE, B::VERTEX | B::COPY_DST | B::COPY_SRC | extra);
            let gpu = self.gpu.then(|| {
                let state = fresh(*alloc, bake.states[p]);
                let state = init(bytemuck::cast_slice(&state), B::STORAGE | B::COPY_DST | B::VERTEX);
                let sims = buffer(r, CHUNK as u64 * SIM, B::UNIFORM | B::COPY_DST);
                let size = wgpu::BufferSize::new(size_of::<Sim>() as u64);
                let sim =
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &sims, offset: 0, size });
                let host = bake.hosts[p].and_then(|h| stores[h].gpu.as_ref()).map_or(&self.none, |g| &g.0);
                let whole = [&particles, &state, &points, host].map(|b| b.as_entire_binding());
                let resources = whole.into_iter().chain([sim]).enumerate();
                let entries: Vec<_> = resources
                    .map(|(b, resource)| wgpu::BindGroupEntry { binding: b as u32, resource })
                    .collect();
                let desc =
                    wgpu::BindGroupDescriptor { label: None, layout: &self.layouts[0], entries: &entries };
                let group = r.device.create_bind_group(&desc);
                (state, sims, group)
            });
            let k = u64::from(bake.trails[p]);
            let trail =
                (!self.gpu && k > 0).then(|| buffer(r, u64::from(*alloc) * k * 16, B::VERTEX | B::COPY_DST));
            let mut orders: Vec<(Stack, f32)> = Vec::new();
            for (o, size) in self.kits.iter().filter(|k| k.system == bake.systems[p]).filter_map(|k| k.order)
            {
                match orders.iter_mut().find(|(e, _)| *e == o) {
                    Some(e) => e.1 = e.1.max(size),
                    None => orders.push((o, size)),
                }
            }
            let stacks =
                orders.into_iter().map(|(o, size)| self.stacked(r, (&particles, *alloc), o, size)).collect();
            stores.push(Store { particles, gpu, trail, stacks });
        }
        let clouds = if self.gpu { Vec::new() } else { bake.allocs.iter().map(|a| Cloud::new(*a)).collect() };
        Flight { stores, clouds }
    }

    pub(crate) fn trim(&mut self) {
        self.flights.iter_mut().for_each(|s| *s = None);
    }

    pub(crate) fn slots(&self) -> u32 {
        self.flights.iter().zip(&self.herds).filter(|(s, _)| s.is_some()).map(|(_, h)| h.bake.alloc()).sum()
    }
}

impl Renderer {
    pub fn census(&self, view: &SceneView) -> Result<Vec<(String, Census)>, RenderError> {
        let Some(swarms) = &view.swarms else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (herd, flight) in swarms.herds.iter().zip(&swarms.flights) {
            let Some(flight) = flight else { continue };
            let system = herd.bake.systems[0];
            let id =
                swarms.kits.iter().find(|k| k.system == system).map(|k| k.id.clone()).unwrap_or_default();
            if !flight.clouds.is_empty() {
                let parts: Vec<Census> = flight.clouds.iter().map(|c| Census::of(&c.particles)).collect();
                out.push((id, Census::join(&parts)));
                continue;
            }
            let mut parts = Vec::new();
            for store in &flight.stores {
                let size = store.particles.size();
                let staging = buffer(self, size, B::MAP_READ | B::COPY_DST);
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(&store.particles, 0, &staging, 0, size);
                let done = self.queue.submit([encoder.finish()]);
                self.map(&staging.slice(..), &Default::default(), done)?;
                let data =
                    staging.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
                parts.push(Census::of(bytemuck::cast_slice(&data)));
            }
            out.push((id, Census::join(&parts)));
        }
        Ok(out)
    }
}
