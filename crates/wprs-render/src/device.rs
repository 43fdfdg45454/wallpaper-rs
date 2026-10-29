use crate::develop::Developer;
use crate::readback::{MAX_READBACK_BYTES, TargetLimits};
use crate::shader::{Pairs, bind_group_layout, module};
use crate::texture::{GpuImage, create_image};
use crate::{RenderError, log_allowed};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

pub type Display = Box<dyn wgpu::wgt::WgpuHasDisplayHandle>;

fn ready<F: Future>(work: F) -> F::Output {
    let mut work = std::pin::pin!(work);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match work.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => pollster::block_on(work),
    }
}
type Lost = Arc<(AtomicBool, Mutex<Option<Box<dyn Fn(String) + Send>>>)>;
static SHARED: Mutex<Option<&'static Renderer>> = Mutex::new(None);

pub struct Renderer {
    pub(crate) instance: wgpu::Instance,
    pub(crate) gpu: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) layers: Pairs,
    pub(crate) clamp: wgpu::Sampler,
    pub(crate) repeat: wgpu::Sampler,
    pub(crate) placeholder: GpuImage,
    adapter: String,
    pub(crate) limits: TargetLimits,
    pub(crate) developer: OnceLock<Developer>,
    pub(crate) reliefs: OnceLock<Pairs>,
    pub(crate) scene: OnceLock<crate::scene::Pipes>,
    pub(crate) yuv: OnceLock<crate::yuv::Converter>,
    pub(crate) luma: OnceLock<crate::luma::Luma>,
    pub(crate) uploads: crate::scene::Uploads,
    lost: Lost,
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        Self::with_display(None)
    }

    pub fn with_adapter(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Self, RenderError> {
        let offered = adapter.limits();
        let required_limits = wgpu::Limits {
            max_texture_dimension_2d: offered.max_texture_dimension_2d,
            max_buffer_size: offered.max_buffer_size,
            ..wgpu::Limits::downlevel_defaults()
        };
        let descriptor =
            wgpu::DeviceDescriptor { label: Some("wprs-render"), required_limits, ..Default::default() };
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))
            .map_err(|e| RenderError::Device(e.to_string()))?;
        let lost: Lost = Arc::default();
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                flag.0.store(true, Ordering::Relaxed);
                let call = flag.1.lock().unwrap_or_else(PoisonError::into_inner);
                call.iter().for_each(|call| call(format!("{reason:?}: {message}")));
            }
        });
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
            layers: Pairs::new(layout, module(&device, include_str!("shader.wgsl")), "fs_main"),
            clamp: sampler(wgpu::AddressMode::ClampToEdge),
            repeat: sampler(wgpu::AddressMode::Repeat),
            placeholder: create_image(&device, &queue, &[0; 4], 1, 1, 4),
            adapter: format!("{} ({:?})", info.name, info.backend),
            limits,
            developer: OnceLock::new(),
            reliefs: OnceLock::new(),
            scene: OnceLock::new(),
            yuv: OnceLock::new(),
            luma: OnceLock::new(),
            uploads: Default::default(),
            lost,
            device,
            queue,
            instance,
            gpu: adapter,
        })
    }

    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn shared() -> Result<&'static Renderer, RenderError> {
        let mut shared = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
        match *shared {
            Some(r) if !r.lost() => Ok(r),
            _ => Ok(*shared.insert(Box::leak(Box::new(Renderer::new()?)))),
        }
    }

    pub fn install(self) -> &'static Renderer {
        let installed: &'static Renderer = Box::leak(Box::new(self));
        let mut shared = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.is_none_or(Renderer::lost) {
            *shared = Some(installed);
        }
        installed
    }

    pub fn lost(&self) -> bool {
        self.lost.0.load(Ordering::Relaxed)
    }

    pub fn on_device_lost(&self, callback: impl Fn(String) + Send + 'static) {
        *self.lost.1.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(callback));
    }

    pub(crate) fn scoped<T>(&self, work: impl FnOnce() -> T) -> Result<T, RenderError> {
        let memory = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let value = work();
        let failure = [validation.pop(), memory.pop()].into_iter().find_map(ready);
        failure.map_or(Ok(value), |e| Err(RenderError::Gpu(e.to_string())))
    }
}
