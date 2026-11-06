use crate::Renderer;
use crate::texture::{GpuImage, gpu_image};
use wgpu::TextureUsages as U;

const SIDE: u32 = 256;

impl Renderer {
    pub(crate) fn env_map(&self, source: &GpuImage) -> GpuImage {
        let exact = self.exact(source);
        let source = exact.as_ref().unwrap_or(source);
        let levels = 32 - SIDE.leading_zeros();
        let format = crate::packed::drawable(source.texture.format());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wprs-render solid env"),
            size: wgpu::Extent3d { width: SIDE, height: SIDE / 2, depth_or_array_layers: 1 },
            mip_level_count: levels - 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: U::TEXTURE_BINDING | U::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let pipes = self.pipes();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for level in 0..levels - 1 {
            let from = match level {
                0 => source.view.clone(),
                _ => texture.create_view(&super::super::mips::level(level - 1)),
            };
            let to = texture.create_view(&super::super::mips::level(level));
            pipes.shrink(&self.device, &mut encoder, &from, &to, format);
        }
        self.queue.submit([encoder.finish()]);
        gpu_image(texture)
    }
}
