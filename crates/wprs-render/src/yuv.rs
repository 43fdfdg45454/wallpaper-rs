use crate::coded::Coding;
use crate::shader::{bind_group_layout, module, pipeline};
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use wgpu::{BindingResource as Resource, TextureFormat as Format, TextureUsages as Usage};

pub struct Planes<'a> {
    pub size: (u32, u32),
    pub luma: (&'a [u8], u32),
    pub chroma: (&'a [u8], u32),
    pub bt709: bool,
    pub full: bool,
    pub coding: Coding,
    pub headroom: f32,
}

type Key = ((u32, u32), bool, bool, Coding, u32);

pub struct Yuv {
    key: Key,
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
    coded: wgpu::RenderPipeline,
    lit: wgpu::RenderPipeline,
}

const TURNS: [[f32; 4]; 8] = [
    [1.0, 0.0, 0.0, 1.0],
    [0.0, 1.0, -1.0, 0.0],
    [-1.0, 0.0, 0.0, -1.0],
    [0.0, -1.0, 1.0, 0.0],
    [-1.0, 0.0, 0.0, 1.0],
    [0.0, -1.0, -1.0, 0.0],
    [1.0, 0.0, 0.0, -1.0],
    [0.0, 1.0, 1.0, 0.0],
];

pub(crate) fn uniform(renderer: &Renderer, bt709: bool, full: bool, turn: u8) -> wgpu::Buffer {
    let turn = TURNS[usize::from(turn) % TURNS.len()];
    let values = rows(bt709, full).into_iter().chain([turn]).flatten();
    let contents: Vec<u8> = values.flat_map(f32::to_le_bytes).collect();
    let usage = wgpu::BufferUsages::UNIFORM;
    crate::filled::filled((&renderer.device, &renderer.queue), "wprs-render yuv rows", &contents, usage)
}

pub(crate) fn group(
    renderer: &Renderer,
    rows: &wgpu::Buffer,
    planes: [&wgpu::Texture; 2],
) -> wgpu::BindGroup {
    let views = planes.map(|t| t.create_view(&Default::default()));
    let resources = [
        rows.as_entire_binding(),
        Resource::TextureView(&views[0]),
        Resource::TextureView(&views[1]),
        Resource::Sampler(&renderer.clamp),
    ];
    let entries: Vec<_> = (resources.into_iter().enumerate())
        .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
        .collect();
    let layout = &renderer.converter().layout;
    let label = Some("wprs-render yuv group");
    renderer.device.create_bind_group(&wgpu::BindGroupDescriptor { label, layout, entries: &entries })
}

pub(crate) fn rows(bt709: bool, full: bool) -> [[f32; 4]; 3] {
    rows_of(if bt709 { (0.2126, 0.0722) } else { (0.299, 0.114) }, full)
}

pub(crate) fn rows_of((kr, kb): (f32, f32), full: bool) -> [[f32; 4]; 3] {
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
    pub(crate) fn converter(&self) -> &Converter {
        self.yuv.get_or_init(|| {
            let layout = bind_group_layout(&self.device, 2);
            let made = |source, format: Format| {
                let shader = module(&self.device, source);
                pipeline(&self.device, &layout, &shader, "fs_main", format.into())
            };
            let (pipeline, coded) = (
                made(include_str!("yuv.wgsl"), Format::Rgba8UnormSrgb),
                made(crate::coded::source(), Format::Rgba8UnormSrgb),
            );
            let lit = made(crate::coded::source(), Format::Rgba16Float);
            Converter { layout, pipeline, coded, lit }
        })
    }

    fn yuv_slot(&self, key: Key) -> Yuv {
        let ((w, h), bt709, full, coding, light) = key;
        let headroom = f32::from_bits(light);
        let (sampled, drawn) =
            (Usage::TEXTURE_BINDING | Usage::COPY_DST, Usage::TEXTURE_BINDING | Usage::COPY_SRC);
        let luma = create_texture(&self.device, "wprs-render luma", (w, h), Format::R8Unorm, sampled);
        let half = (w.div_ceil(2), h.div_ceil(2));
        let chroma = create_texture(&self.device, "wprs-render chroma", half, Format::Rg8Unorm, sampled);
        let target = if headroom > 1.0 { Format::Rgba16Float } else { Format::Rgba8UnormSrgb };
        let image = gpu_image(create_texture(
            &self.device,
            "wprs-render video",
            (w, h),
            target,
            drawn | Usage::RENDER_ATTACHMENT,
        ));
        let rows = match coding {
            Coding::Sdr => uniform(self, bt709, full, 0),
            hdr => crate::coded::uniform(self, full, hdr, headroom),
        };
        let group = group(self, &rows, [&luma, &chroma]);
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
        let light = crate::coded::lighting(planes.coding, planes.headroom).to_bits();
        let key = (planes.size, planes.bt709, planes.full, planes.coding, light);
        if slot.as_ref().is_none_or(|yuv| yuv.key != key) {
            *slot = Some(self.scoped(|| self.yuv_slot(key))?);
        }
        let yuv = slot.as_mut().expect("created above");
        self.scoped(|| {
            write(&self.queue, &yuv.luma, planes.luma);
            write(&self.queue, &yuv.chroma, planes.chroma);
            match (yuv.key.3, f32::from_bits(yuv.key.4) > 1.0) {
                (Coding::Sdr, _) => self.convert(&yuv.group, &yuv.image),
                (_, false) => self.draw_with(&self.converter().coded, &yuv.group, &yuv.image),
                (_, true) => self.draw_with(&self.converter().lit, &yuv.group, &yuv.image),
            }
        })?;
        Ok(&yuv.image)
    }

    pub(crate) fn convert(&self, group: &wgpu::BindGroup, image: &GpuImage) {
        self.draw_with(&self.converter().pipeline, group, image)
    }

    pub(crate) fn draw_with(
        &self,
        pipeline: &wgpu::RenderPipeline,
        group: &wgpu::BindGroup,
        image: &GpuImage,
    ) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let ops =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
        let view = &image.view;
        let attachment =
            wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wprs-render yuv pass"),
            color_attachments: &[Some(attachment)],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        self.queue.submit([encoder.finish()]);
    }
}
