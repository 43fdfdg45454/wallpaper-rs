use crate::parallax::zoom;
use crate::texture::GpuImage;
use crate::{Layer, Renderer};
use bytemuck::{Pod, Zeroable};
use wprs_core::{Fit, UvRect};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
struct Params {
    offset: [f32; 2],
    scale: [f32; 2],
    background: [f32; 4],
    flags: [u32; 4],
    shift: [f32; 2],
    focus: f32,
    steps: f32,
    grade: [[f32; 4]; 5],
}

pub(crate) struct Slot {
    uniform: wgpu::Buffer,
    params: Params,
    pub(crate) group: wgpu::BindGroup,
    key: (u64, bool, Option<u64>),
}

impl Slot {
    pub(crate) fn deep(&self) -> bool {
        self.key.2.is_some()
    }
}

impl Renderer {
    pub(crate) fn bind(&self, slots: &mut Vec<Slot>, index: usize, layer: &Layer<'_>, target: (u32, u32)) {
        let uv = layer.image.map_or(UvRect::compute(Fit::Stretch, (1, 1), (1, 1)), |image| {
            layer.uv.unwrap_or_else(|| UvRect::compute(layer.fit, image.size(), target))
        });
        let relief = layer.relief.filter(|_| layer.image.is_some());
        let uv = flipped(relief.map_or(uv, |r| zoom(uv, r.overscan)), layer.flip);
        let turned = |s: [f32; 2]| [0, 1].map(|i| if layer.flip[i] { -s[i] } else { s[i] });
        let [r, g, b] = layer.background.map(srgb_to_linear);
        let params = Params {
            offset: uv.offset,
            scale: uv.scale,
            background: [r, g, b, 1.0],
            flags: [
                uv.repeat as u32,
                layer.image.is_some() as u32,
                layer.image.is_some_and(opaque) as u32,
                0,
            ],
            shift: relief.map_or([0.0; 2], |r| turned(r.shift)),
            focus: relief.map_or(0.0, |r| r.focus),
            steps: relief.map_or(1, |r| r.steps.max(1)) as f32,
            grade: layer.grade.map_or([[0.0; 4]; 5], |g| g.uniform()),
        };
        let (image, depth) = (layer.image.unwrap_or(&self.placeholder), relief.map(|r| r.depth));
        let key = (image.id, uv.repeat, depth.map(|d| d.id));
        let Some(slot) = slots.get_mut(index) else {
            let usage = wgpu::BufferUsages::UNIFORM;
            let devices = (&self.device, &self.queue);
            let uniform =
                crate::filled::filled(devices, "wprs-render params", bytemuck::bytes_of(&params), usage);
            let group = self.group(&uniform, image, uv.repeat, depth);
            return slots.push(Slot { uniform, params, group, key });
        };
        if slot.params != params {
            self.queue.write_buffer(&slot.uniform, 0, bytemuck::bytes_of(&params));
            slot.params = params;
        }
        if slot.key != key {
            slot.group = self.group(&slot.uniform, image, uv.repeat, depth);
            slot.key = key;
        }
    }

    fn group(
        &self,
        uniform: &wgpu::Buffer,
        image: &GpuImage,
        repeat: bool,
        depth: Option<&GpuImage>,
    ) -> wgpu::BindGroup {
        let sampler = if repeat && depth.is_none() { &self.repeat } else { &self.clamp };
        let layout = if depth.is_some() { &self.reliefs().layout } else { &self.layers.layout };
        let views = std::iter::once(image).chain(depth).map(|i| wgpu::BindingResource::TextureView(&i.view));
        let resources = std::iter::once(uniform.as_entire_binding())
            .chain(views)
            .chain([wgpu::BindingResource::Sampler(sampler)]);
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render bind group"),
            layout,
            entries: &entries,
        })
    }
}

fn flipped(mut uv: UvRect, flip: [bool; 2]) -> UvRect {
    for i in (0..2).filter(|i| flip[*i]) {
        (uv.offset[i], uv.scale[i]) = (uv.offset[i] + uv.scale[i], -uv.scale[i]);
    }
    uv
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn opaque(image: &GpuImage) -> bool {
    image.texture.format() == wgpu::TextureFormat::Bgra8UnormSrgb
}

#[cfg(test)]
mod tests {
    use crate::{Layer, Renderer};
    use wprs_core::Fit;

    #[test]
    fn bind_groups_and_uniforms_persist_until_the_layer_changes() {
        let r = match Renderer::shared() {
            Ok(r) => r,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let red = r.upload_rgba(&[255, 0, 0, 255].repeat(4), 2, 2, 8).unwrap();
        let blue = r.upload_rgba(&[0, 0, 255, 255].repeat(4), 2, 2, 8).unwrap();
        let mut target = r.create_target(16, 8).unwrap();
        let mut pixels = Vec::new();
        let layer = |image, fit, opacity| Layer {
            image: Some(image),
            fit,
            background: [0; 3],
            opacity,
            uv: None,
            relief: None,
            flip: [false; 2],
            grade: None,
        };
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
