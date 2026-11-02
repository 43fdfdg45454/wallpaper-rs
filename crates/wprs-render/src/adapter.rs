use crate::device::Display;
use crate::{RenderError, Renderer};
use std::sync::Arc;
use wgpu::rwh::{DisplayHandle, HandleError, HasDisplayHandle};
use wgpu::wgt::WgpuHasDisplayHandle;
use wgpu::{Backends, DeviceType};

#[derive(Debug)]
struct Shared(Arc<dyn WgpuHasDisplayHandle>);

impl HasDisplayHandle for Shared {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

pub(crate) fn instance(
    backends: Backends,
    display: Option<&Arc<dyn WgpuHasDisplayHandle>>,
) -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = backends;
    if backends == Backends::GL {
        desc.flags.remove(wgpu::InstanceFlags::VALIDATION);
    }
    desc.display = display.map(|d| Box::new(Shared(d.clone())) as Display);
    wgpu::Instance::new(desc)
}

fn open(
    backends: Backends,
    display: Option<&Arc<dyn WgpuHasDisplayHandle>>,
) -> Option<(wgpu::Instance, wgpu::Adapter)> {
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    };
    let instance = instance(backends, display);
    let adapter = pollster::block_on(instance.request_adapter(&options));
    adapter
        .inspect_err(|e| tracing::debug!(?backends, "no adapter: {e}"))
        .ok()
        .map(|adapter| (instance, adapter))
}

fn software(found: &Option<(wgpu::Instance, wgpu::Adapter)>) -> bool {
    found.as_ref().is_none_or(|(_, a)| a.get_info().device_type == DeviceType::Cpu)
}

pub(crate) fn pick(first: Option<DeviceType>, then: impl FnOnce() -> Option<DeviceType>) -> bool {
    let soft = |t: Option<DeviceType>| t.is_none_or(|t| t == DeviceType::Cpu);
    soft(first) && !soft(then()) || first.is_none()
}

pub(crate) fn open_auto(
    display: Option<&Arc<dyn WgpuHasDisplayHandle>>,
) -> Result<(wgpu::Instance, wgpu::Adapter), RenderError> {
    if let Some(chosen) = Backends::from_env() {
        return open(chosen, display).ok_or_else(|| RenderError::NoAdapter(format!("{chosen:?}")));
    }
    let vulkan = open(Backends::VULKAN, display);
    let gl = software(&vulkan).then(|| open(Backends::GL, display)).flatten();
    let kind = |found: &Option<(wgpu::Instance, wgpu::Adapter)>| {
        found.as_ref().map(|(_, a)| a.get_info().device_type)
    };
    let found = if pick(kind(&vulkan), || kind(&gl)) { gl.or(vulkan) } else { vulkan.or(gl) };
    found.ok_or_else(|| RenderError::NoAdapter("neither Vulkan nor OpenGL ES".into()))
}

impl Renderer {
    pub fn with_display(display: Option<Display>) -> Result<Self, RenderError> {
        let display: Option<Arc<dyn WgpuHasDisplayHandle>> = display.map(Arc::from);
        let (instance, adapter) = open_auto(display.as_ref())?;
        Self::with_adapter(instance, adapter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opengl_is_only_used_when_vulkan_has_no_hardware_adapter() {
        let (cpu, gpu) = (Some(DeviceType::Cpu), Some(DeviceType::IntegratedGpu));
        assert!(!pick(gpu, || panic!("OpenGL was opened although Vulkan has a GPU")));
        assert!(pick(cpu, || gpu) && !pick(cpu, || cpu) && !pick(cpu, || None));
        assert!(pick(None, || gpu) && pick(None, || None));
    }
}
