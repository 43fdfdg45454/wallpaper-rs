use crate::texture::GpuImage;
use crate::{Layer, Renderer};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wprs_core::{Fit, UvRect};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
struct Params {
    offset: [f32; 2],
    scale: [f32; 2],
    background: [f32; 4],
    flags: [u32; 4],
}

pub(crate) struct Slot {
    uniform: wgpu::Buffer,
    params: Params,
    pub(crate) group: wgpu::BindGroup,
    key: (u64, bool),
}

impl Renderer {
    pub(crate) fn bind(&self, slots: &mut Vec<Slot>, index: usize, layer: &Layer<'_>, target: (u32, u32)) {
        let uv = layer.image.map_or(UvRect::compute(Fit::Stretch, (1, 1), (1, 1)), |image| {
            layer.uv.unwrap_or_else(|| UvRect::compute(layer.fit, image.size(), target))
        });
        let [r, g, b] = layer.background.map(srgb_to_linear);
        let params = Params {
            offset: uv.offset,
            scale: uv.scale,
            background: [r, g, b, 1.0],
            flags: [uv.repeat as u32, layer.image.is_some() as u32, 0, 0],
        };
        let image = layer.image.unwrap_or(&self.placeholder);
        let key = (image.id, uv.repeat);
        let Some(slot) = slots.get_mut(index) else {
            let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("wprs-render params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let group = self.group(&uniform, image, uv.repeat);
            return slots.push(Slot { uniform, params, group, key });
        };
        if slot.params != params {
            self.queue.write_buffer(&slot.uniform, 0, bytemuck::bytes_of(&params));
            slot.params = params;
        }
        if slot.key != key {
            slot.group = self.group(&slot.uniform, image, uv.repeat);
            slot.key = key;
        }
    }

    fn group(&self, uniform: &wgpu::Buffer, image: &GpuImage, repeat: bool) -> wgpu::BindGroup {
        let sampler = if repeat { &self.repeat } else { &self.clamp };
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&image.view),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        })
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[cfg(test)]
mod tests {
    use crate::{Layer, Renderer};
    use wprs_core::Fit;

    #[test]
    fn bind_groups_and_uniforms_persist_until_the_layer_changes() {
        let r = match Renderer::new() {
            Ok(r) => r,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let red = r.upload_rgba(&[255, 0, 0, 255].repeat(4), 2, 2, 8).unwrap();
        let blue = r.upload_rgba(&[0, 0, 255, 255].repeat(4), 2, 2, 8).unwrap();
        let mut target = r.create_target(16, 8).unwrap();
        let mut pixels = Vec::new();
        let layer =
            |image, fit, opacity| Layer { image: Some(image), fit, background: [0; 3], opacity, uv: None };
        let mut draw = |layers: &[Layer<'_>]| {
            r.render_layers(&mut target, layers, &mut pixels).unwrap();
            target.slots.iter().map(|s| (s.group.clone(), s.uniform.clone(), s.params)).collect::<Vec<_>>()
        };
        let first = draw(&[layer(&red, Fit::Fill, 1.0)]);
        let again = draw(&[layer(&red, Fit::Fill, 1.0)]);
        assert!(first[0].0 == again[0].0 && first[0].1 == again[0].1);
        let refit = draw(&[layer(&red, Fit::Fit, 1.0)]);
        assert!(refit[0].0 == first[0].0 && refit[0].1 == first[0].1 && refit[0].2 != first[0].2);
        let fading = draw(&[layer(&red, Fit::Fit, 1.0), layer(&blue, Fit::Fit, 0.5)]);
        assert!(fading.len() == 2 && fading[0].0 == first[0].0);
        let faded = draw(&[layer(&red, Fit::Fit, 1.0), layer(&blue, Fit::Fit, 0.7)]);
        assert!(faded[1].0 == fading[1].0);
        let swapped = draw(&[layer(&blue, Fit::Fit, 1.0)]);
        assert!(swapped.len() == 1 && swapped[0].0 != first[0].0 && swapped[0].1 == first[0].1);
    }
}
