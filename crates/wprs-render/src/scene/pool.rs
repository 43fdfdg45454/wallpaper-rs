use super::pipes::{Pipes, STRIDE};
use super::state::volume;
use crate::texture::{create_texture, next_id};
use std::collections::HashMap;
use wgpu::{BindGroup, TextureFormat, TextureUsages as U};

const KEEP_FRAMES: u64 = 120;
pub(crate) const BUDGET: u64 = 512 << 20;

#[derive(Clone)]
pub(crate) struct Tex {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) id: u64,
}

pub(crate) type Key = [u64; 8];

pub(crate) struct Pool {
    free: Vec<(Tex, u64)>,
    pub(crate) busy: Vec<(Tex, bool, u64)>,
    groups: HashMap<Key, (BindGroup, u64)>,
    pub(crate) data: Vec<u8>,
    buffer: Option<wgpu::Buffer>,
    frame_group: Option<(BindGroup, BindGroup)>,
    pub(crate) white: Tex,
    pub(crate) clear: Tex,
    pub(crate) void: Tex,
    pub(crate) frame: u64,
    pub(crate) created: u32,
    bytes: u64,
}

impl Tex {
    pub(crate) fn new(device: &wgpu::Device, (w, h): (u32, u32), format: TextureFormat) -> Tex {
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC | U::COPY_DST;
        let texture = create_texture(device, "wprs-scene texture", (w.max(1), h.max(1)), format, usage);
        Tex { view: texture.create_view(&Default::default()), texture, id: next_id() }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    pub(crate) fn bytes(&self) -> u64 {
        bytes(self.size(), self.texture.format())
    }
}

fn bytes((w, h): (u32, u32), format: TextureFormat) -> u64 {
    u64::from(w) * u64::from(h) * u64::from(format.block_copy_size(None).unwrap_or(8))
}

impl Pool {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: TextureFormat) -> Pool {
        let solid = |rgba: [u8; 4]| {
            let tex = Tex::new(device, (1, 1), TextureFormat::Rgba8Unorm);
            let layout =
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) };
            queue.write_texture(tex.texture.as_image_copy(), &rgba, layout, tex.texture.size());
            tex
        };
        let _ = format;
        let (free, busy, groups, data) = Default::default();
        let (white, clear, void) = (solid([255; 4]), solid([0; 4]), volume(device, queue, 1, &[[0.0; 3]]));
        let (buffer, frame_group, frame, created, bytes) = (None, None, 0, 3, 0);
        Pool { free, busy, groups, data, buffer, frame_group, white, clear, void, frame, created, bytes }
    }

    pub(crate) fn acquire(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
    ) -> Option<usize> {
        let size = (size.0.max(1), size.1.max(1));
        if self.bytes + bytes(size, format) > BUDGET {
            return None;
        }
        self.bytes += bytes(size, format);
        let fits = |t: &Tex| t.size() == size && t.texture.format() == format;
        if let Some(i) = self.busy.iter().position(|(t, used, _)| !used && fits(t)) {
            (self.busy[i].1, self.busy[i].2) = (true, self.frame);
            return Some(i);
        }
        let tex = self.take(device, size, format);
        self.busy.push((tex, true, self.frame));
        Some(self.busy.len() - 1)
    }

    pub(crate) fn release(&mut self, i: usize) {
        if let Some((tex, used, _)) = self.busy.get_mut(i).filter(|(_, used, _)| *used) {
            *used = false;
            self.bytes -= tex.bytes();
        }
    }

    pub(crate) fn take(&mut self, device: &wgpu::Device, size: (u32, u32), format: TextureFormat) -> Tex {
        let found = self.free.iter().position(|(t, _)| t.size() == size && t.texture.format() == format);
        match found {
            Some(i) => self.free.remove(i).0,
            None => {
                self.created += 1;
                Tex::new(device, size, format)
            }
        }
    }

    pub(crate) fn give(&mut self, tex: Tex) {
        self.free.push((tex, self.frame));
    }

    pub(crate) fn push<T: bytemuck::Pod>(&mut self, value: &T) -> u32 {
        let offset = self.data.len();
        self.data.extend_from_slice(bytemuck::bytes_of(value));
        self.data.resize(offset + STRIDE as usize, 0);
        offset as u32
    }

    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipes: &Pipes,
        frame: &wgpu::Buffer,
    ) {
        let needed = (self.data.len() as u64).max(STRIDE);
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

    pub(crate) fn group(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        fx: bool,
        views: &[(u64, &wgpu::TextureView)],
    ) -> BindGroup {
        let mut key = [0; 8];
        key[0] = u64::from(fx);
        views.iter().zip(&mut key[1..]).for_each(|((id, _), k)| *k = *id);
        if let Some((group, used)) = self.groups.get_mut(&key) {
            *used = self.frame;
            return group.clone();
        }
        let buffer = self.buffer.as_ref().expect("uploaded before encoding");
        let uniform = wgpu::BufferBinding { buffer, offset: 0, size: wgpu::BufferSize::new(STRIDE) };
        let first = fx.then_some(wgpu::BindingResource::Buffer(uniform));
        let resources =
            first.into_iter().chain(views.iter().map(|(_, v)| wgpu::BindingResource::TextureView(v)));
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let group =
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries });
        self.created += 1;
        self.groups.insert(key, (group.clone(), self.frame));
        group
    }

    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        self.busy.iter_mut().for_each(|slot| slot.1 = false);
        if self.busy.iter().any(|(.., used)| frame - used >= KEEP_FRAMES) {
            let idle = self.busy.extract_if(.., |(.., used)| frame - *used >= KEEP_FRAMES);
            self.free.extend(idle.map(|(t, _, used)| (t, used)));
        }
        self.free.sort_unstable_by_key(|(t, used)| (std::cmp::Reverse(*used), t.id));
        let (mut kept, mut gone) = (0, Vec::new());
        self.free.retain(|(t, used)| {
            kept += t.bytes();
            let keep = frame - used < KEEP_FRAMES && kept <= BUDGET;
            if !keep {
                gone.push(t.id);
            }
            keep
        });
        let stale = |key: &Key| key[1..].iter().any(|id| *id != 0 && gone.contains(id));
        self.groups.retain(|key, (_, used)| frame - *used < KEEP_FRAMES && !stale(key));
        self.data.clear();
        (self.frame, self.bytes) = (frame + 1, 0);
    }

    pub(crate) fn trim(&mut self) {
        self.free.clear();
        self.busy.clear();
        self.groups.clear();
    }

    pub(crate) fn textures(&self) -> usize {
        self.free.len() + self.busy.len()
    }
}
