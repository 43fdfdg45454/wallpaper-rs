use crate::texture::{GpuImage, gpu_image, texture};
use std::collections::HashMap;
use std::sync::Mutex;
use wgpu::TextureFormat as Format;

mod env;
mod gpu;
mod layout;
mod looks;
mod paint;
mod pipes;
mod staged;
mod uniform;

pub(crate) use gpu::GpuModel;
pub(crate) use staged::Staged;

pub(crate) const DEPTH: Format = Format::Depth32Float;
pub(crate) const SCENE_SLOT: u64 = 1024;
pub(crate) const SLOT: u64 = 256;

pub(crate) struct Solids {
    pub(crate) layouts: [wgpu::BindGroupLayout; 4],
    pub(crate) pipes: Mutex<HashMap<u16, wgpu::RenderPipeline>>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) wrap: wgpu::Sampler,
    pub(crate) white: GpuImage,
    pub(crate) flat: GpuImage,
}

fn sampler(device: &wgpu::Device, v: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("wprs-render solids"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: v,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

impl crate::Renderer {
    pub(crate) fn solids(&self) -> &Solids {
        self.solids.get_or_init(|| {
            let pixel = |rgba: [u8; 4], format| gpu_image(texture(self, &rgba, (1, 1), 4, format));
            Solids {
                layouts: layout::layouts(&self.device),
                pipes: Mutex::new(HashMap::new()),
                sampler: sampler(&self.device, wgpu::AddressMode::Repeat),
                wrap: sampler(&self.device, wgpu::AddressMode::ClampToEdge),
                white: pixel([255; 4], Format::Rgba8UnormSrgb),
                flat: pixel([128, 128, 255, 255], Format::Rgba8Unorm),
            }
        })
    }

    pub(crate) fn solid_format(&self, hdr: bool) -> Format {
        let half = self.gpu.get_texture_format_features(Format::Rgba16Float);
        let flags = wgpu::TextureFormatFeatureFlags::FILTERABLE | wgpu::TextureFormatFeatureFlags::BLENDABLE;
        let usable = half.allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            && half.flags.contains(flags);
        if hdr && usable { Format::Rgba16Float } else { Format::Rgba8UnormSrgb }
    }
}
