use super::gpu::{Gpu, Load, Variant};
use wgpu::{Features, TextureFormat};

pub(super) struct Set {
    views: Vec<wgpu::TextureView>,
    groups: Vec<wgpu::BindGroup>,
    pipe: wgpu::RenderPipeline,
    stamps: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
}

fn noise(format: TextureFormat, texels: usize, mut seed: u32) -> Vec<u8> {
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    match format {
        TextureFormat::Rgba16Float => {
            (0..texels * 4).flat_map(|_| (0x3800 | (next() & 0x3ff) as u16).to_le_bytes()).collect()
        }
        _ => (0..texels).flat_map(|_| next().to_le_bytes()).collect(),
    }
}

impl Gpu {
    pub(super) fn set(&mut self, v: &Variant, (w, h): (u32, u32), passes: usize) -> Set {
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST;
        let bpp = v.format.block_copy_size(None).unwrap_or(4);
        let views: Vec<wgpu::TextureView> = (0..3)
            .map(|k| {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: v.format,
                    usage,
                    view_formats: &[],
                });
                let data = noise(v.format, (w * h) as usize, 0x9e37_79b9 ^ k);
                let layout = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * bpp),
                    rows_per_image: None,
                };
                self.queue.write_texture(texture.as_image_copy(), &data, layout, size);
                texture.create_view(&Default::default())
            })
            .collect();
        let groups = views
            .iter()
            .map(|view| {
                let entries = [
                    wgpu::BindGroupEntry { binding: 0, resource: self.unused.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ];
                let desc = wgpu::BindGroupDescriptor { label: None, layout: &self.layout, entries: &entries };
                self.device.create_bind_group(&desc)
            })
            .collect();
        let stamps = (v.stamps && self.has(Features::TIMESTAMP_QUERY)).then(|| {
            let count = passes as u32 * 2;
            let ty = wgpu::QueryType::Timestamp;
            let set = self.device.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty, count });
            let buffer = |usage| {
                let size = u64::from(count) * 8;
                let desc = wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false };
                self.device.create_buffer(&desc)
            };
            let resolve = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
            (set, resolve, buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ))
        });
        Set { views, groups, pipe: self.pipe(v), stamps }
    }

    pub(super) fn frame(&self, set: &Set, v: &Variant, passes: usize) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let rounds = if v.one_pass { 1 } else { passes };
        for p in 0..rounds {
            let group = if v.fixed { &set.groups[2] } else { &set.groups[p % 2] };
            let timestamp_writes =
                set.stamps.as_ref().map(|(query_set, ..)| wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(p as u32 * 2),
                    end_of_pass_write_index: Some(p as u32 * 2 + 1),
                });
            let load = match v.load {
                Load::Clear => wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                Load::Keep => wgpu::LoadOp::Load,
                Load::DontCare => wgpu::LoadOp::DontCare(unsafe { wgpu::LoadOpDontCare::enabled() }),
            };
            let store = if v.store { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard };
            let attachment = wgpu::RenderPassColorAttachment {
                view: &set.views[(p + 1) % 2],
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store },
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(attachment)],
                timestamp_writes,
                ..Default::default()
            });
            pass.set_pipeline(&set.pipe);
            pass.set_bind_group(0, group, &[]);
            (0..if v.one_pass { passes } else { 1 }).for_each(|_| pass.draw(0..3, 0..1));
            drop(pass);
            if v.split && p + 1 < rounds {
                let done =
                    std::mem::replace(&mut encoder, self.device.create_command_encoder(&Default::default()));
                self.queue.submit([done.finish()]);
            }
        }
        if let Some((query_set, resolve, read)) = &set.stamps {
            encoder.resolve_query_set(query_set, 0..rounds as u32 * 2, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, read, 0, rounds as u64 * 16);
        }
        self.queue.submit([encoder.finish()]);
    }

    pub(super) fn stamped(&self, set: &Set, rounds: usize) -> Option<(f64, f64)> {
        let (_, _, read) = set.stamps.as_ref()?;
        let slice = read.slice(..rounds as u64 * 16);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.wait().ok()?;
        let period = f64::from(self.queue.get_timestamp_period()) / 1e6;
        let ticks: Vec<u64> = slice
            .get_mapped_range()
            .ok()?
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect();
        read.unmap();
        let sum: u64 = ticks.as_chunks::<2>().0.iter().map(|[a, b]| b.saturating_sub(*a)).sum();
        let span = ticks.last()?.saturating_sub(*ticks.first()?);
        Some((sum as f64 * period, span as f64 * period))
    }
}
