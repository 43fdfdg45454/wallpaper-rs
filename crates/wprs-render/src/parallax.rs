use crate::shader::{bind_group_layout, module, pair};
use crate::stack::plan;
use crate::texture::{GpuImage, gpu_image, texture};
use crate::{RenderError, Renderer};
use half::f16;
use std::sync::{Mutex, PoisonError};
use wgpu::{TextureFormat as Format, TextureUsages as Usage};
use wprs_core::{Edit, UvRect};

pub type DepthMap = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

#[derive(Clone, Copy)]
pub struct Relief<'a> {
    pub depth: &'a GpuImage,
    pub shift: [f32; 2],
    pub focus: f32,
    pub steps: u32,
    pub overscan: f32,
}

pub(crate) struct Reliefs {
    pub(crate) layout: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    pipelines: Mutex<Vec<(Format, [wgpu::RenderPipeline; 2])>>,
}

pub(crate) fn zoom(uv: UvRect, overscan: f32) -> UvRect {
    let z = overscan.max(1.0);
    let offset = uv.offset.map(|o| 0.5 + (o - 0.5) / z);
    UvRect { offset, scale: uv.scale.map(|s| s / z), repeat: false }
}

pub(crate) fn halves(map: &DepthMap) -> Vec<u16> {
    map.as_raw().iter().map(|v| f16::from_f32(f32::from(*v) / 65535.0).to_bits()).collect()
}

impl Renderer {
    pub(crate) fn reliefs(&self) -> &Reliefs {
        self.reliefs.get_or_init(|| Reliefs {
            layout: bind_group_layout(&self.device, 2),
            shader: module(&self.device, include_str!("parallax.wgsl")),
            pipelines: Mutex::default(),
        })
    }

    pub(crate) fn relief_pipelines(&self, format: Format) -> [wgpu::RenderPipeline; 2] {
        let r = self.reliefs();
        let mut cached = r.pipelines.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, made)) = cached.iter().find(|(f, _)| *f == format) {
            return made.clone();
        }
        let made = pair(&self.device, &r.layout, &r.shader, "fs_parallax", format);
        cached.push((format, made.clone()));
        made
    }

    pub fn upload_depth(&self, map: &DepthMap, edit: Option<&Edit>) -> Result<GpuImage, RenderError> {
        let (size, halves) = (map.dimensions(), halves(map));
        self.check_target(size.0, size.1)?;
        self.scoped(|| {
            let raw = texture(self, bytemuck::cast_slice(&halves), size, size.0 * 2, Format::R16Float);
            let Some(edit) = edit.filter(|e| !e.geometry.is_identity()) else { return gpu_image(raw) };
            let d = self.developer();
            let frame = edit.geometry.frame(size);
            let turned = Edit { geometry: edit.geometry.clone(), ..Edit::default() };
            let stack = plan(&turned, size, frame, false).stack;
            let (out, view) =
                (self.target(frame, d.format, Usage::empty()), raw.create_view(&Default::default()));
            let mut encoder = self.device.create_command_encoder(&Default::default());
            self.pass(&mut encoder, &d.develop, &d.layout, bytemuck::bytes_of(&stack), &[&view; 5], &out);
            self.queue.submit([encoder.finish()]);
            gpu_image(out)
        })
    }
}
