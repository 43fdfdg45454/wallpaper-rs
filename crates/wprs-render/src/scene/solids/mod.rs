use crate::texture::{GpuImage, gpu_image, texture};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use wgpu::TextureFormat as Format;

mod calls;
mod env;
mod gpu;
mod groups;
mod layout;
mod looks;
mod mix;
mod package;
mod paint;
mod pass;
mod pipes;
mod shadows;
mod staged;
mod uniform;

pub(crate) use gpu::GpuModel;
pub(crate) use staged::Staged;

pub(crate) const DEPTH: Format = Format::Depth32Float;
pub(crate) const SCENE_SLOT: u64 = 1024;
pub(crate) const SLOT: u64 = 256;
pub(crate) const FINISH_SLOT: u64 = 512;
pub(crate) const BONES_SLOT: u64 = 12288;
pub(crate) const SOURCE: [&str; 3] =
    [include_str!("solid_head.wgsl"), include_str!("solid.wgsl"), include_str!("solid_light.wgsl")];

pub(crate) struct Solids {
    pub(crate) layouts: [wgpu::BindGroupLayout; 4],
    pub(crate) pipes: Mutex<HashMap<(u16, u64), wgpu::RenderPipeline>>,
    pub(crate) packages: Mutex<HashMap<u64, Option<wgpu::ShaderModule>>>,
    pub(crate) module: OnceLock<wgpu::ShaderModule>,
    pub(crate) mixer: OnceLock<mix::Mixer>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) wrap: wgpu::Sampler,
    pub(crate) compare: wgpu::Sampler,
    pub(crate) white: GpuImage,
    pub(crate) flat: GpuImage,
    pub(crate) no_shadow: wgpu::TextureView,
    pub(crate) no_morph: wgpu::TextureView,
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

impl Solids {
    pub(crate) fn module(&self, device: &wgpu::Device) -> wgpu::ShaderModule {
        let source = format!(
            "{}\nfn material(s: Surface) -> Surface {{\n    return s;\n}}\n{}",
            wprs_scene::STAGES_WGSL,
            SOURCE.concat()
        );
        self.module.get_or_init(|| crate::shader::module(device, &source)).clone()
    }
}

impl crate::Renderer {
    pub(crate) fn solids(&self) -> &Solids {
        self.solids.get_or_init(|| {
            let pixel = |rgba: [u8; 4], format| gpu_image(texture(self, &rgba, (1, 1), 4, format));
            let compare = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wprs-render shadows"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            });
            Solids {
                layouts: layout::layouts(&self.device),
                pipes: Mutex::new(HashMap::new()),
                packages: Mutex::new(HashMap::new()),
                module: OnceLock::new(),
                mixer: OnceLock::new(),
                sampler: sampler(&self.device, wgpu::AddressMode::Repeat),
                wrap: sampler(&self.device, wgpu::AddressMode::ClampToEdge),
                compare,
                white: pixel([255; 4], Format::Rgba8UnormSrgb),
                flat: pixel([128, 128, 255, 255], Format::Rgba8Unorm),
                no_shadow: shadows::blank(self),
                no_morph: gpu::blank_morphs(self),
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
