use super::super::pipes::{Pipes, STRIDE};
use super::Pool;
use wgpu::BindGroup;

impl Pool {
    pub(crate) fn push<T: bytemuck::Pod>(&mut self, value: &T) -> u32 {
        let offset = self.data.len();
        self.data.extend_from_slice(bytemuck::bytes_of(value));
        let room = std::mem::size_of::<T>().div_ceil(STRIDE as usize) * STRIDE as usize;
        self.data.resize(offset + room.max(STRIDE as usize), 0);
        offset as u32
    }

    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipes: &Pipes,
        frame: &wgpu::Buffer,
    ) {
        let needed = (self.data.len() as u64).max(STRIDE) + STRIDE;
        if self.buffer.as_ref().is_none_or(|b| b.size() < needed) {
            let size = needed.next_power_of_two().max(64 * STRIDE);
            let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wprs-scene uniforms"),
                size,
                usage,
                mapped_at_creation: false,
            });
            let entries = |items: bool| {
                let item =
                    wgpu::BufferBinding { buffer: &buffer, offset: 0, size: wgpu::BufferSize::new(STRIDE) };
                let mut list = vec![
                    wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipes.sampler),
                    },
                ];
                let resource = match items {
                    true => wgpu::BindingResource::Buffer(item),
                    false => wgpu::BindingResource::Sampler(&pipes.wrap),
                };
                list.push(wgpu::BindGroupEntry { binding: 2, resource });
                if items {
                    let near = wgpu::BindingResource::Sampler(&pipes.near);
                    list.push(wgpu::BindGroupEntry { binding: 3, resource: near });
                }
                list
            };
            let group = |layout, items| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout,
                    entries: &entries(items),
                })
            };
            self.frame_group = Some((group(&pipes.frame, true), group(&pipes.fx_frame, false)));
            self.buffer = Some(buffer);
            self.groups.clear();
            self.created += 3;
        }
        if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, &self.data);
        }
    }

    pub(crate) fn frame_groups(&self) -> &(BindGroup, BindGroup) {
        self.frame_group.as_ref().expect("uploaded before encoding")
    }
}
