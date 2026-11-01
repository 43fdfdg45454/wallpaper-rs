use crate::shader::{module, pipeline};
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use wgpu::{BindingResource as Resource, TextureFormat as Format, TextureUsages as Usage};

pub const REFERENCE_NITS: f32 = 203.0;
pub const PEAK_NITS: f32 = 1000.0;
pub const KNEE: f32 = 0.5;
pub const GAMUT: [[f32; 4]; 3] =
    [[1.6605, -0.5876, -0.0728, 0.0], [-0.1246, 1.1329, -0.0083, 0.0], [-0.0182, -0.1006, 1.1187, 0.0]];

pub struct DeepPlanes<'a> {
    pub size: (u32, u32),
    pub planes: [(&'a [u8], u32); 3],
    pub hlg: bool,
    pub full: bool,
}

pub struct Deep {
    key: ((u32, u32), bool, bool),
    planes: [wgpu::Texture; 3],
    group: wgpu::BindGroup,
    image: GpuImage,
}

pub(crate) struct Mapper {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

pub fn deep_rows(full: bool) -> [[f32; 4]; 3] {
    let (kr, kb) = (0.2627, 0.0593);
    let kg = 1.0 - kr - kb;
    let (ys, yo, cs) = if full { (1.0, 0.0, 1.0) } else { (1023.0 / 876.0, 64.0 / 1023.0, 1023.0 / 896.0) };
    let row = |u: f32, v: f32| [ys, u * cs, v * cs, -ys * yo - cs * 512.0 / 1023.0 * (u + v)];
    let green = row(-2.0 * kb * (1.0 - kb) / kg, -2.0 * kr * (1.0 - kr) / kg);
    [row(0.0, 2.0 * (1.0 - kr)), green, row(2.0 * (1.0 - kb), 0.0)]
}

fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Uint,
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let entries: Vec<_> = std::iter::once(uniform)
        .chain([texture; 3])
        .enumerate()
        .map(|(i, ty)| wgpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("wprs-render deep"),
        entries: &entries,
    })
}

fn fits((data, stride): (&[u8], u32), (w, h): (u32, u32)) -> bool {
    stride >= w * 2
        && data.len() as u64 >= u64::from(stride) * u64::from(h.saturating_sub(1)) + u64::from(w * 2)
}

impl Renderer {
    pub(crate) fn mapper(&self) -> &Mapper {
        self.deep.get_or_init(|| {
            let layout = layout(&self.device);
            let shader = module(&self.device, include_str!("deep.wgsl"));
            let pipeline = pipeline(&self.device, &layout, &shader, "fs_main", Format::Rgba8UnormSrgb.into());
            Mapper { layout, pipeline }
        })
    }

    fn deep_slot(&self, key: ((u32, u32), bool, bool)) -> Deep {
        let ((w, h), hlg, full) = key;
        let half = (w.div_ceil(2), h.div_ceil(2));
        let sampled = Usage::TEXTURE_BINDING | Usage::COPY_DST;
        let plane =
            |size| create_texture(&self.device, "wprs-render deep plane", size, Format::R16Uint, sampled);
        let planes = [plane((w, h)), plane(half), plane(half)];
        let usage = Usage::TEXTURE_BINDING | Usage::COPY_SRC | Usage::RENDER_ATTACHMENT;
        let image = gpu_image(create_texture(
            &self.device,
            "wprs-render deep video",
            (w, h),
            Format::Rgba8UnormSrgb,
            usage,
        ));
        let tone = [if hlg { 2.0 } else { 1.0 }, REFERENCE_NITS, PEAK_NITS, KNEE];
        let values = deep_rows(full).into_iter().chain([tone]).chain(GAMUT).flatten();
        let contents: Vec<u8> = values.flat_map(f32::to_le_bytes).collect();
        let devices = (&self.device, &self.queue);
        let uniform =
            crate::filled::filled(devices, "wprs-render deep rows", &contents, wgpu::BufferUsages::UNIFORM);
        let views = planes.each_ref().map(|t| t.create_view(&Default::default()));
        let resources =
            std::iter::once(uniform.as_entire_binding()).chain(views.iter().map(Resource::TextureView));
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let layout = &self.mapper().layout;
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wprs-render deep"),
            layout,
            entries: &entries,
        });
        Deep { key, planes, group, image }
    }

    pub fn convert_deep<'a>(
        &self,
        slot: &'a mut Option<Deep>,
        deep: &DeepPlanes<'_>,
    ) -> Result<&'a GpuImage, RenderError> {
        let (w, h) = deep.size;
        let half = (w.div_ceil(2), h.div_ceil(2));
        let sizes = [(w, h), half, half];
        if !self.limits.fits(w, h) || !deep.planes.iter().zip(sizes).all(|(p, s)| fits(*p, s)) {
            return Err(RenderError::InvalidSize(w, h));
        }
        let key = (deep.size, deep.hlg, deep.full);
        if slot.as_ref().is_none_or(|d| d.key != key) {
            *slot = Some(self.scoped(|| self.deep_slot(key))?);
        }
        let made = slot.as_mut().expect("created above");
        self.scoped(|| {
            for (texture, (data, stride)) in made.planes.iter().zip(deep.planes) {
                let layout = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                };
                self.queue.write_texture(texture.as_image_copy(), data, layout, texture.size());
            }
            self.draw_with(&self.mapper().pipeline, &made.group, &made.image);
        })?;
        Ok(&made.image)
    }
}
