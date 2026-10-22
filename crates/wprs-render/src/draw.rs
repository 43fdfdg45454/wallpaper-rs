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
        let rendered = self.scoped(|| {
            let size = target.size();
            let pipelines = (&self.pipelines, TARGET_FORMAT);
            let mut encoder = self.encode(&target.view, &mut target.slots, layers, size, pipelines);
            let layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes(size.0)),
                rows_per_image: Some(size.1),
            };
            let copy = wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout };
            encoder.copy_texture_to_buffer(target.texture.as_image_copy(), copy, target.texture.size());
            let submission = self.queue.submit([encoder.finish()]);
            self.read(target, submission, read)
        });
        rendered.flatten().inspect_err(|_| target.slots.clear())
    }

    pub(crate) fn encode(
        &self,
        view: &wgpu::TextureView,
        slots: &mut Vec<Slot>,
        layers: &[Layer<'_>],
        size: (u32, u32),
        (flat, format): (&[wgpu::RenderPipeline; 2], wgpu::TextureFormat),
    ) -> wgpu::CommandEncoder {
        let visible = || layers.iter().filter(|l| l.opacity > 0.0);
        visible().enumerate().for_each(|(i, layer)| self.bind(slots, i, layer, size));
        slots.truncate(visible().count());
        let deep = slots.iter().any(Slot::deep).then(|| self.relief_pipelines(format));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let ops =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
        let attachment =
            wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render pass"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        for (i, (slot, layer)) in slots.iter().zip(visible()).enumerate() {
            let fading = i > 0 && layer.opacity < 1.0;
            let [opaque, blended] = deep.as_ref().filter(|_| slot.deep()).unwrap_or(flat);
            pass.set_pipeline(if fading { blended } else { opaque });
            if fading {
                let c = f64::from(layer.opacity);
                pass.set_blend_constant(wgpu::Color { r: c, g: c, b: c, a: c });
            }
            pass.set_bind_group(0, &slot.group, &[]);
            pass.draw(0..3, 0..1);
        }
        drop(pass);
        encoder
    }
}
