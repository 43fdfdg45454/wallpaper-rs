use crate::device::create_texture;
use crate::{Frame, Layer, RenderError, Renderer, Scene};

pub(crate) const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
}

impl Target {
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }
}

impl Renderer {
    pub fn create_target(&self, width: u32, height: u32) -> Result<Target, RenderError> {
        self.check_size(width, height)?;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        let texture =
            create_texture(&self.device, "wprs-render target", (width, height), TARGET_FORMAT, usage);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-render readback"),
            size: u64::from(padded_row_bytes(width)) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Target { view: texture.create_view(&Default::default()), texture, readback })
    }

    pub fn render(&self, scene: Scene<'_>, width: u32, height: u32) -> Result<Frame, RenderError> {
        let target = self.create_target(width, height)?;
        let image = scene.image.map(|img| self.upload(img, scene.fit, (width, height)));
        let layer =
            Layer { image: image.as_ref(), fit: scene.fit, background: scene.background, opacity: 1.0 };
        self.render_layers(&target, &[layer])
    }

    pub fn render_layers(&self, target: &Target, layers: &[Layer<'_>]) -> Result<Frame, RenderError> {
        let visible = layers.iter().filter(|l| l.opacity > 0.0);
        let groups: Vec<_> =
            visible.map(|l| (self.bind_group(l, target.size()), l.opacity.min(1.0))).collect();
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
        for (i, (group, opacity)) in groups.iter().enumerate() {
            let fading = i > 0 && *opacity < 1.0;
            pass.set_pipeline(if fading { &self.blended } else { &self.opaque });
            if fading {
                let c = f64::from(*opacity);
                pass.set_blend_constant(wgpu::Color { r: c, g: c, b: c, a: c });
            }
            pass.set_bind_group(0, group, &[]);
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
        self.queue.submit([encoder.finish()]);
        self.read(target)
    }

    fn read(&self, target: &Target) -> Result<Frame, RenderError> {
        let (width, height) = target.size();
        let failed = |e: &dyn std::fmt::Display| RenderError::Readback(e.to_string());
        let slice = target.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| failed(&e))?;
        rx.recv().map_err(|e| failed(&e))?.map_err(|e| failed(&e))?;
        let stride = width * 4;
        let data = slice
            .get_mapped_range()
            .map_err(|e| failed(&e))?
            .chunks_exact(padded_row_bytes(width) as usize)
            .flat_map(|row| &row[..stride as usize])
            .copied()
            .collect();
        target.readback.unmap();
        Ok(Frame { width, height, stride, data })
    }
}

fn padded_row_bytes(width: u32) -> u32 {
    (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}
