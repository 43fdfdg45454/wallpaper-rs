use crate::shader::{bind_group_layout, module, pipeline};
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindingResource as Resource, TextureFormat as Format, TextureUsages as Usage};

pub struct Planes<'a> {
    pub size: (u32, u32),
    pub luma: (&'a [u8], u32),
    pub chroma: (&'a [u8], u32),
    pub bt709: bool,
    pub full: bool,
}

pub struct Yuv {
    key: ((u32, u32), bool, bool),
    luma: wgpu::Texture,
    chroma: wgpu::Texture,
    group: wgpu::BindGroup,
    image: GpuImage,
}

impl Yuv {
    pub fn image(&self) -> &GpuImage {
        &self.image
    }
}

pub(crate) struct Converter {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

fn rows(bt709: bool, full: bool) -> [[f32; 4]; 3] {
    let (kr, kb) = if bt709 { (0.2126, 0.0722) } else { (0.299, 0.114) };
    let kg = 1.0 - kr - kb;
    let (ys, yo, cs) = if full { (1.0, 0.0, 1.0) } else { (255.0 / 219.0, 16.0 / 255.0, 255.0 / 224.0) };
    let row = |u: f32, v: f32| [ys, u * cs, v * cs, -ys * yo - cs * 128.0 / 255.0 * (u + v)];
    let green = row(-2.0 * kb * (1.0 - kb) / kg, -2.0 * kr * (1.0 - kr) / kg);
    [row(0.0, 2.0 * (1.0 - kr)), green, row(2.0 * (1.0 - kb), 0.0)]
}

fn covers((data, stride): (&[u8], u32), (width, height): (u32, u32), texel: u32) -> bool {
    let needed = u64::from(stride) * u64::from(height.saturating_sub(1)) + u64::from(width * texel);
    stride >= width * texel && data.len() as u64 >= needed
}

fn write(queue: &wgpu::Queue, texture: &wgpu::Texture, (data, stride): (&[u8], u32)) {
    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None };
    queue.write_texture(texture.as_image_copy(), data, layout, texture.size());
}

impl Renderer {
    fn converter(&self) -> &Converter {
        self.yuv.get_or_init(|| {
            let layout = bind_group_layout(&self.device, 2);
            let shader = module(&self.device, include_str!("yuv.wgsl"));
            let pipeline = pipeline(&self.device, &layout, &shader, "fs_main", Format::Rgba8UnormSrgb.into());
            Converter { layout, pipeline }
        })
    }

    fn yuv_slot(&self, key: ((u32, u32), bool, bool)) -> Yuv {
        let ((w, h), bt709, full) = key;
        let (sampled, drawn) =
            (Usage::TEXTURE_BINDING | Usage::COPY_DST, Usage::TEXTURE_BINDING | Usage::COPY_SRC);
        let luma = create_texture(&self.device, "wprs-render luma", (w, h), Format::R8Unorm, sampled);
        let half = (w.div_ceil(2), h.div_ceil(2));
        let chroma = create_texture(&self.device, "wprs-render chroma", half, Format::Rg8Unorm, sampled);
        let target = Format::Rgba8UnormSrgb;
        let image = gpu_image(create_texture(
            &self.device,
            "wprs-render video",
            (w, h),
            target,
            drawn | Usage::RENDER_ATTACHMENT,
        ));
        let contents: Vec<u8> = rows(bt709, full).iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let label = Some("wprs-render yuv rows");
        let usage = wgpu::BufferUsages::UNIFORM;
        let buffer =
            self.device.create_buffer_init(&BufferInitDescriptor { label, contents: &contents, usage });
        let views = [&luma, &chroma].map(|t| t.create_view(&Default::default()));
        let resources = [
            buffer.as_entire_binding(),
            Resource::TextureView(&views[0]),
            Resource::TextureView(&views[1]),
            Resource::Sampler(&self.clamp),
        ];
        let entries: Vec<_> = (resources.into_iter().enumerate())
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let layout = &self.converter().layout;
        let group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor { label, layout, entries: &entries });
        Yuv { key, luma, chroma, group, image }
    }

    pub fn convert_yuv<'a>(
        &self,
        slot: &'a mut Option<Yuv>,
        planes: &Planes<'_>,
    ) -> Result<&'a GpuImage, RenderError> {
        let (w, h) = planes.size;
        let half = (w.div_ceil(2), h.div_ceil(2));
        if !self.limits.fits(w, h) || !covers(planes.luma, (w, h), 1) || !covers(planes.chroma, half, 2) {
            return Err(RenderError::InvalidSize(w, h));
        }
        let key = (planes.size, planes.bt709, planes.full);
        if slot.as_ref().is_none_or(|yuv| yuv.key != key) {
            *slot = Some(self.scoped(|| self.yuv_slot(key))?);
        }
        let yuv = slot.as_mut().expect("created above");
        self.scoped(|| {
            write(&self.queue, &yuv.luma, planes.luma);
            write(&self.queue, &yuv.chroma, planes.chroma);
            let mut encoder = self.device.create_command_encoder(&Default::default());
            let ops = wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            };
            let view = &yuv.image.view;
            let attachment =
                wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wprs-render yuv pass"),
                color_attachments: &[Some(attachment)],
                ..Default::default()
            });
            pass.set_pipeline(&self.converter().pipeline);
            pass.set_bind_group(0, &yuv.group, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            self.queue.submit([encoder.finish()]);
        })?;
        Ok(&yuv.image)
    }
}

#[cfg(test)]
mod tests {
    use super::rows;

    fn near(bt709: bool, full: bool, [y, u, v]: [u8; 3], want: [i32; 3]) -> bool {
        let yuv = [y, u, v].map(|c| f32::from(c) / 255.0);
        let rgb = rows(bt709, full).map(|r| {
            ((r[0] * yuv[0] + r[1] * yuv[1] + r[2] * yuv[2] + r[3]).clamp(0.0, 1.0) * 255.0).round() as i32
        });
        rgb.iter().zip(want).all(|(have, want)| (have - want).abs() <= 2)
    }

    #[test]
    fn studio_and_full_swing_colours_come_back() {
        assert!(near(false, false, [81, 90, 240], [255, 0, 0]));
        assert!(near(true, false, [63, 102, 240], [255, 0, 0]));
        assert!(near(true, false, [173, 42, 26], [0, 255, 0]));
        assert!(near(true, false, [235, 128, 128], [255, 255, 255]));
        assert!(near(false, true, [0, 128, 128], [0, 0, 0]));
        assert!(near(true, true, [128, 128, 128], [128, 128, 128]));
        assert!(!near(false, false, [63, 102, 240], [255, 0, 0]));
    }
}
