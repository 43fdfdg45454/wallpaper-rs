use super::super::Image;
use crate::Renderer;
use crate::texture::{gpu_image, texture};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::BufferUsages as B;
use wgpu::TextureFormat as Format;
use wprs_scene::Model;

pub(crate) struct GpuModel {
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) maps: Vec<wgpu::BindGroup>,
}

impl Renderer {
    pub(crate) fn texture_cap(&self) -> u32 {
        let phone = cfg!(target_os = "android");
        wprs_scene::model_caps(self.tier(), phone).texture.min(self.max_edge())
    }

    fn model_picture(&self, bytes: &[u8], linear: bool) -> Option<Image> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
        let rgba = crate::decode_limited(reader, crate::picture_limits()).ok()?.into_rgba8();
        let prepared = crate::decode::scaled(&rgba, 1.0, self.texture_cap());
        let (w, h) = prepared.dimensions();
        let format = if linear { Format::Rgba8Unorm } else { Format::Rgba8UnormSrgb };
        let gpu = gpu_image(texture(self, prepared.as_raw(), (w, h), w * 4, format));
        Some(self.mipped(&Arc::new((gpu, [w as f32, h as f32])), None))
    }

    pub(crate) fn gpu_model(&self, model: &Model) -> GpuModel {
        let fill = |label, bytes: &[u8], usage| {
            crate::filled::filled((&self.device, &self.queue), label, bytes, usage)
        };
        let vertices = fill("wprs-render solid vertices", bytemuck::cast_slice(&model.vertices), B::VERTEX);
        let indices = fill("wprs-render solid indices", bytemuck::cast_slice(&model.indices), B::INDEX);
        let solids = self.solids();
        let mut made: HashMap<(u16, bool), Option<Image>> = HashMap::new();
        let mut picture = |slot: Option<u16>, linear: bool| -> Option<Image> {
            let t = slot?;
            made.entry((t, linear))
                .or_insert_with(|| self.model_picture(&model.textures.get(usize::from(t))?.bytes, linear))
                .clone()
        };
        let mut maps = Vec::with_capacity(model.materials.len() + 1);
        let defaults = wprs_scene::Finish::default();
        for m in model.materials.iter().chain([&defaults]) {
            let linear = [false, true, true, true, false];
            let found: Vec<Option<Image>> = (0..5).map(|k| picture(m.maps[k], linear[k])).collect();
            let fallback = |k: usize| if k == 2 { &solids.flat.view } else { &solids.white.view };
            let views: Vec<&wgpu::TextureView> =
                (0..5).map(|k| found[k].as_ref().map_or(fallback(k), |i| &i.0.view)).collect();
            let mut entries: Vec<wgpu::BindGroupEntry> = views
                .iter()
                .enumerate()
                .map(|(k, v)| wgpu::BindGroupEntry {
                    binding: k as u32,
                    resource: wgpu::BindingResource::TextureView(v),
                })
                .collect();
            entries.push(wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&solids.sampler),
            });
            maps.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wprs-render solid maps"),
                layout: &solids.layouts[2],
                entries: &entries,
            }));
        }
        GpuModel { vertices, indices, maps }
    }
}
