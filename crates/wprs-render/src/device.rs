use crate::RenderError;
use crate::shader::{bind_group_layout, pipelines};
use crate::texture::{GpuImage, create_image};

pub struct Renderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) opaque: wgpu::RenderPipeline,
    pub(crate) blended: wgpu::RenderPipeline,
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) clamp: wgpu::Sampler,
    pub(crate) repeat: wgpu::Sampler,
    pub(crate) placeholder: GpuImage,
    adapter: String,
    max_edge: u32,
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL);
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        };
        let adapter = wgpu::Instance::new(desc)
            .request_adapter(&options)
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let max_edge = adapter.limits().max_texture_dimension_2d;
        let required_limits =
            wgpu::Limits { max_texture_dimension_2d: max_edge, ..wgpu::Limits::downlevel_defaults() };
        let descriptor =
            wgpu::DeviceDescriptor { label: Some("wprs-render"), required_limits, ..Default::default() };
        let (device, queue) =
            adapter.request_device(&descriptor).await.map_err(|e| RenderError::Device(e.to_string()))?;
        let layout = bind_group_layout(&device);
        let [opaque, blended] = pipelines(&device, &layout);
        let sampler = |mode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wprs-render sampler"),
                address_mode_u: mode,
                address_mode_v: mode,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        };
        let info = adapter.get_info();
        Ok(Self {
            opaque,
            blended,
            clamp: sampler(wgpu::AddressMode::ClampToEdge),
            repeat: sampler(wgpu::AddressMode::Repeat),
            placeholder: create_image(&device, &queue, &[0; 4], 1, 1, 4),
            adapter: format!("{} ({:?})", info.name, info.backend),
            max_edge,
            layout,
            device,
            queue,
        })
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn max_edge(&self) -> u32 {
        self.max_edge
    }

    pub(crate) fn check_size(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let fits = |edge| (1..=self.max_edge).contains(&edge);
        (fits(width) && fits(height)).then_some(()).ok_or(RenderError::InvalidSize(width, height))
    }
}

pub(crate) fn create_texture(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
