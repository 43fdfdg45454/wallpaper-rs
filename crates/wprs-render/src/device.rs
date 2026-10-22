use crate::develop::Developer;
use crate::draw::TARGET_FORMAT;
use crate::parallax::Reliefs;
use crate::readback::{MAX_READBACK_BYTES, TargetLimits};
use crate::shader::{bind_group_layout, pipelines};
use crate::texture::{GpuImage, create_image};
use crate::{RenderError, log_allowed};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, OnceLock};

pub type Display = Box<dyn wgpu::wgt::WgpuHasDisplayHandle>;

pub struct Renderer {
    pub(crate) instance: wgpu::Instance,
    pub(crate) gpu: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) pipelines: [wgpu::RenderPipeline; 2],
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) clamp: wgpu::Sampler,
    pub(crate) repeat: wgpu::Sampler,
    pub(crate) placeholder: GpuImage,
    adapter: String,
    pub(crate) limits: TargetLimits,
    pub(crate) developer: OnceLock<Developer>,
    pub(crate) reliefs: OnceLock<Reliefs>,
    pub(crate) scene: OnceLock<crate::scene::Pipes>,
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        Self::with_display(None)
    }

    pub fn with_display(display: Option<Display>) -> Result<Self, RenderError> {
        pollster::block_on(Self::new_async(display))
    }

    async fn new_async(display: Option<Display>) -> Result<Self, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL);
        desc.display = display;
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        };
        let instance = wgpu::Instance::new(desc);
        let adapter =
            instance.request_adapter(&options).await.map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let offered = adapter.limits();
        let required_limits = wgpu::Limits {
            max_texture_dimension_2d: offered.max_texture_dimension_2d,
            max_buffer_size: offered.max_buffer_size,
            ..wgpu::Limits::downlevel_defaults()
        };
        let descriptor =
            wgpu::DeviceDescriptor { label: Some("wprs-render"), required_limits, ..Default::default() };
        let (device, queue) =
            adapter.request_device(&descriptor).await.map_err(|e| RenderError::Device(e.to_string()))?;
        static GPU_ERRORS: AtomicU64 = AtomicU64::new(0);
        device.on_uncaptured_error(Arc::new(|e| {
            if log_allowed(&GPU_ERRORS) {
                tracing::error!("GPU error: {e}");
            }
        }));
        let granted = device.limits();
        let limits = TargetLimits {
            max_edge: granted.max_texture_dimension_2d,
            max_buffer_size: granted.max_buffer_size.min(MAX_READBACK_BYTES),
        };
        let layout = bind_group_layout(&device, 1);
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
            pipelines: pipelines(&device, &layout, TARGET_FORMAT),
            clamp: sampler(wgpu::AddressMode::ClampToEdge),
            repeat: sampler(wgpu::AddressMode::Repeat),
            placeholder: create_image(&device, &queue, &[0; 4], 1, 1, 4),
            adapter: format!("{} ({:?})", info.name, info.backend),
            limits,
            developer: OnceLock::new(),
            reliefs: OnceLock::new(),
            scene: OnceLock::new(),
            layout,
            device,
            queue,
            instance,
            gpu: adapter,
        })
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn on_device_lost(&self, callback: impl Fn(String) + Send + 'static) {
        self.device.set_device_lost_callback(move |reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                callback(format!("{reason:?}: {message}"));
            }
        });
    }

    pub(crate) fn scoped<T>(&self, work: impl FnOnce() -> T) -> Result<T, RenderError> {
        let memory = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let value = work();
        let failure = [validation.pop(), memory.pop()].into_iter().find_map(pollster::block_on);
        failure.map_or(Ok(value), |e| Err(RenderError::Gpu(e.to_string())))
    }
}
