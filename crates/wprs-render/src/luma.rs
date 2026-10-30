use crate::readback::{FAILED, MAPPED, Mapped};
use crate::shader::{bind_group_layout, module, pipeline};
use crate::{GpuImage, RenderError, Renderer};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use wprs_scene::FLASH_GRID;

const ROW: u32 = 256;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(crate) struct Luma {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    grid: wgpu::Buffer,
}

pub struct Meter {
    target: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
    state: Arc<Mapped>,
    pending: bool,
    group: Option<(u64, wgpu::BindGroup)>,
}

impl Luma {
    fn new(devices: (&wgpu::Device, &wgpu::Queue)) -> Luma {
        let device = devices.0;
        let layout = bind_group_layout(device, 1);
        let shader = module(device, include_str!("luma.wgsl"));
        let pipeline = pipeline(device, &layout, &shader, "fs_luma", FORMAT.into());
        let cells = [FLASH_GRID[0] as f32, FLASH_GRID[1] as f32, 0.0, 0.0];
        let usage = wgpu::BufferUsages::UNIFORM;
        let grid = crate::filled::filled(devices, "wprs-render luma grid", bytemuck::bytes_of(&cells), usage);
        Luma { layout, pipeline, grid }
    }
}

impl Renderer {
    pub fn meter(&self) -> Meter {
        let [width, height] = FLASH_GRID;
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wprs-render luma"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-render luma readback"),
            size: u64::from(ROW * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = target.create_view(&Default::default());
        Meter { target, view, buffer, state: Arc::default(), pending: false, group: None }
    }

    pub fn luma(
        &self,
        m: &mut Meter,
        image: Option<&GpuImage>,
        out: &mut Vec<f32>,
    ) -> Result<bool, RenderError> {
        let read = m.pending && self.collect(m, out)?;
        match image {
            Some(image) if !m.pending => self.measure(m, image).map(|()| read),
            _ => Ok(read),
        }
    }

    fn collect(&self, m: &mut Meter, out: &mut Vec<f32>) -> Result<bool, RenderError> {
        self.polled(wgpu::PollType::Poll)?;
        match m.state.load(Relaxed) {
            MAPPED => {}
            FAILED => {
                m.pending = false;
                return Err(RenderError::Readback("cannot map the luminance buffer".into()));
            }
            _ => return Ok(false),
        }
        let [w, h] = FLASH_GRID;
        let bytes =
            m.buffer.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
        out.clear();
        out.extend((0..h * w).map(|i| f32::from(bytes[(i / w * ROW + i % w * 4) as usize]) / 255.0));
        drop(bytes);
        m.buffer.unmap();
        m.pending = false;
        Ok(true)
    }

    fn measure(&self, m: &mut Meter, image: &GpuImage) -> Result<(), RenderError> {
        let luma = self.luma.get_or_init(|| Luma::new((&self.device, &self.queue)));
        if m.group.as_ref().is_none_or(|(id, _)| *id != image.id) {
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wprs-render luma"),
                layout: &luma.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: luma.grid.as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&image.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.clamp),
                    },
                ],
            });
            m.group = Some((image.id, group));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let ops = wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            };
            let attachment = wgpu::RenderPassColorAttachment {
                view: &m.view,
                depth_slice: None,
                resolve_target: None,
                ops,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wprs-render luma"),
                color_attachments: &[Some(attachment)],
                ..Default::default()
            });
            pass.set_pipeline(&luma.pipeline);
            pass.set_bind_group(0, m.group.as_ref().map(|(_, g)| g), &[]);
            pass.draw(0..3, 0..1);
        }
        let [width, height] = FLASH_GRID;
        let layout =
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW), rows_per_image: Some(height) };
        let to = wgpu::TexelCopyBufferInfo { buffer: &m.buffer, layout };
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        encoder.copy_texture_to_buffer(m.target.as_image_copy(), to, size);
        self.scoped(|| self.queue.submit([encoder.finish()]))?;
        let flag = m.state.clone();
        m.state.store(0, Relaxed);
        m.buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            flag.store(if r.is_ok() { MAPPED } else { FAILED }, Relaxed)
        });
        m.pending = true;
        Ok(())
    }
}
