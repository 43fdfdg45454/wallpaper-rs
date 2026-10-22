use crate::bind::Slot;
use crate::readback::{copy_rows, padded_row_bytes};
use crate::texture::create_texture;
use crate::{Layer, RenderError, Renderer};
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

pub(crate) const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub struct Target {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) readback: wgpu::Buffer,
    pub(crate) mapped: Arc<AtomicU8>,
    pub(crate) slots: Vec<Slot>,
}

impl Target {
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    pub fn stride(&self) -> u32 {
        self.texture.width() * 4
    }
}

impl Renderer {
    pub fn create_target(&self, width: u32, height: u32) -> Result<Target, RenderError> {
        self.check_target(width, height)?;
        self.scoped(|| {
            let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
            let texture =
                create_texture(&self.device, "wprs-render target", (width, height), TARGET_FORMAT, usage);
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wprs-render readback"),
                size: u64::from(padded_row_bytes(width)) * u64::from(height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let view = texture.create_view(&Default::default());
            Target { texture, view, readback, mapped: Arc::default(), slots: Vec::new() }
        })
    }

    pub fn render_layers(
        &self,
        target: &mut Target,
        layers: &[Layer<'_>],
        dst: &mut Vec<u8>,
    ) -> Result<(), RenderError> {
        self.render_layers_with(target, layers, |width, height, stride, src| {
            copy_rows(src, dst, width, height, stride)
        })
    }

    pub fn render_layers_with<R>(
        &self,
        target: &mut Target,
        layers: &[Layer<'_>],
        read: impl FnOnce(u32, u32, u32, &[u8]) -> R,
    ) -> Result<R, RenderError> {
        let visible = || layers.iter().filter(|l| l.opacity > 0.0);
        let rendered = self.scoped(|| {
            let size = target.size();
            visible().enumerate().for_each(|(i, layer)| self.bind(&mut target.slots, i, layer, size));
            target.slots.truncate(visible().count());
            let submission = self.encode(target, visible().map(|l| l.opacity.min(1.0)));
            self.read(target, submission, read)
        });
        rendered.flatten().inspect_err(|_| target.slots.clear())
    }

    fn encode(&self, target: &Target, opacities: impl Iterator<Item = f32>) -> wgpu::SubmissionIndex {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let ops =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
        let attachment = wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render pass"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        for (i, (slot, opacity)) in target.slots.iter().zip(opacities).enumerate() {
            let fading = i > 0 && opacity < 1.0;
            pass.set_pipeline(if fading { &self.blended } else { &self.opaque });
            if fading {
                let c = f64::from(opacity);
                pass.set_blend_constant(wgpu::Color { r: c, g: c, b: c, a: c });
            }
            pass.set_bind_group(0, &slot.group, &[]);
            pass.draw(0..3, 0..1);
        }
        drop(pass);
        let (width, height) = target.size();
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded_row_bytes(width)),
            rows_per_image: Some(height),
        };
        let copy = wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout };
        encoder.copy_texture_to_buffer(target.texture.as_image_copy(), copy, target.texture.size());
        self.queue.submit([encoder.finish()])
    }
}
