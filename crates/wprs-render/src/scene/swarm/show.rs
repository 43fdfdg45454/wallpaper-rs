use super::{B, LOOK, Look, Swarms, buffer};
use crate::Renderer;
use crate::scene::pipes::Mode;

impl Swarms {
    pub(crate) fn upload(&mut self, r: &Renderer, uniform: &wgpu::Buffer) {
        self.framed(r, uniform);
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
        let pipes = (self.pipelines.get(&mode), self.ribbons.get(&mode));
        let (Some(flight), Some((_, group)), (Some(pipeline), Some(ribbon)), Some(herd)) =
            (flight, &self.looked, pipes, self.herds.get(flock))
        else {
            return;
        };
        let first = (u64::from(slot) / LOOK) as usize;
        for (p, store) in flight.stores.iter().enumerate() {
            let Some(kit) = self.kitted.get(first + p).and_then(|k| self.kits.get(*k)) else { continue };
            let look = self.looks.get(first + p).map_or(0.0, |l| l.extra[1]);
            pass.set_bind_group(0, group, &[slot + (p as u64 * LOOK) as u32]);
            pass.set_bind_group(1, &kit.tex, &[]);
            let (alloc, k) = (u64::from(herd.bake.allocs[p]), u64::from(herd.bake.trails[p]));
            let trail = match (&store.trail, &store.gpu) {
                (Some(t), _) => Some((t, 0)),
                (None, Some((state, ..))) if k > 0 => Some((state, (4 + u64::from(herd.bake.marks[p])) * 4)),
                _ => None,
            };
            if let Some((buffer, at)) = trail.filter(|_| look >= 2.0) {
                pass.set_pipeline(ribbon);
                pass.set_vertex_buffer(0, buffer.slice(at..at + alloc * k * 16 - 16));
                pass.set_vertex_buffer(1, buffer.slice(at + 16..at + alloc * k * 16));
                pass.draw(0..4, 0..(alloc * k - 1) as u32);
            }
            let sorted = kit.order.and_then(|(o, _)| store.stacks.iter().find(|s| s.order == o));
            let own = kit
                .stage
                .as_ref()
                .and_then(|(key, _)| self.stages.get(&(mode, *key)))
                .zip(self.framed.as_ref());
            match own {
                Some((stage, framed)) => {
                    pass.set_pipeline(stage);
                    pass.set_bind_group(2, framed, &[]);
                }
                None => pass.set_pipeline(pipeline),
            }
            pass.set_vertex_buffer(0, sorted.map_or(&store.particles, |s| &s.sorted).slice(..));
            pass.draw(0..4, 0..herd.bake.allocs[p]);
        }
    }
}
