use ndk::native_window::NativeWindow;
use std::sync::OnceLock;
use wgpu::rwh::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};
use wprs_render::{Presentation, RenderError, Renderer, Surface};

static RENDERER: OnceLock<Renderer> = OnceLock::new();

const BLOCKED: &[&str] = &[];

fn blocked(info: &wgpu::AdapterInfo) -> bool {
    BLOCKED.iter().any(|name| info.name.contains(name))
}

fn surface(instance: &wgpu::Instance, window: &NativeWindow) -> Result<wgpu::Surface<'static>, RenderError> {
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(window.ptr().cast())),
    };
    unsafe { instance.create_surface_unsafe(target) }.map_err(|e| RenderError::Surface(e.to_string()))
}

pub(crate) fn open(
    window: &NativeWindow,
    size: (u32, u32),
) -> Result<(&'static Renderer, Surface), RenderError> {
    if let Some(renderer) = RENDERER.get() {
        let surface = surface(renderer.instance(), window)?;
        return Ok((renderer, renderer.configure_surface(surface, size, Presentation::MOBILE)?));
    }
    let mut failure = RenderError::NoAdapter("no backend".into());
    for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
        match create(backends, window, size) {
            Ok((renderer, surface)) => {
                log::info!("rendering with {}", renderer.adapter());
                return Ok((RENDERER.get_or_init(|| renderer), surface));
            }
            Err(e) => {
                log::warn!("{e}");
                failure = e;
            }
        }
    }
    Err(failure)
}

fn create(
    backends: wgpu::Backends,
    window: &NativeWindow,
    size: (u32, u32),
) -> Result<(Renderer, Surface), RenderError> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let surface = surface(&instance, window)?;
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        ..Default::default()
    };
    let adapter = pollster::block_on(instance.request_adapter(&options))
        .map_err(|e| RenderError::NoAdapter(format!("{backends:?}: {e}")))?;
    let info = adapter.get_info();
    if blocked(&info) {
        return Err(RenderError::NoAdapter(format!("{} ({:?}) is blocked", info.name, info.backend)));
    }
    let renderer = Renderer::with_adapter(instance, adapter)?;
    let surface = renderer.configure_surface(surface, size, Presentation::MOBILE)?;
    Ok((renderer, surface))
}
