use crate::bind::Slot;
use crate::draw::TARGET_FORMAT;
use crate::{Layer, RenderError, Renderer};
use wgpu::{CompositeAlphaMode, CurrentSurfaceTexture as Acquired, PresentMode, TextureFormat as Format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    pub vsync: bool,
    pub latency: u32,
    pub format: Option<Format>,
}

impl Presentation {
    pub const DESKTOP: Self = Self { vsync: false, latency: 2, format: Some(TARGET_FORMAT) };
    pub const MOBILE: Self = Self { vsync: true, latency: 1, format: None };
}

pub struct Surface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    presentation: Presentation,
    slots: Vec<Slot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Got {
    Fresh,
    Stale,
    Busy,
    Outdated,
}

fn steps(got: Got) -> (bool, bool) {
    (matches!(got, Got::Fresh | Got::Stale), matches!(got, Got::Stale | Got::Outdated))
}

impl Surface {
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn present_mode(&self) -> PresentMode {
        self.config.present_mode
    }
}

fn present_mode(offered: &[PresentMode], vsync: bool) -> PresentMode {
    let preferred = [PresentMode::Mailbox, PresentMode::Immediate];
    preferred.into_iter().find(|mode| !vsync && offered.contains(mode)).unwrap_or(PresentMode::Fifo)
}

fn surface_format(offered: &[Format], preferred: Option<Format>) -> Option<Format> {
    let srgb = || offered.iter().copied().find(Format::is_srgb);
    preferred.filter(|f| offered.contains(f)).or_else(srgb).or(offered.first().copied())
}

impl Renderer {
    pub fn create_surface(
        &self,
        window: impl wgpu::DisplayAndWindowHandle + 'static,
        size: (u32, u32),
    ) -> Result<Surface, RenderError> {
        self.check_surface(size.0, size.1)?;
        let surface =
            self.instance.create_surface(window).map_err(|e| RenderError::Surface(e.to_string()))?;
        self.configure_surface(surface, size, Presentation::DESKTOP)
    }

    pub fn configure_surface(
        &self,
        surface: wgpu::Surface<'static>,
        (width, height): (u32, u32),
        presentation: Presentation,
    ) -> Result<Surface, RenderError> {
        self.check_surface(width, height)?;
        if !self.gpu.is_surface_supported(&surface) {
            return Err(RenderError::Surface(format!("{} cannot present to this surface", self.adapter())));
        }
        let config = self.surface_config(&surface, (width, height), presentation)?;
        self.scoped(|| surface.configure(&self.device, &config))?;
        Ok(Surface { surface, config, presentation, slots: Vec::new() })
    }

    fn renew(&self, target: &mut Surface) -> Result<(), RenderError> {
        let size = (target.config.width, target.config.height);
        let config = self.surface_config(&target.surface, size, target.presentation)?;
        if config.format != target.config.format {
            target.slots.clear();
        }
        target.config = config;
        self.scoped(|| target.surface.configure(&self.device, &target.config))
    }

    fn surface_config(
        &self,
        surface: &wgpu::Surface<'static>,
        (width, height): (u32, u32),
        presentation: Presentation,
    ) -> Result<wgpu::SurfaceConfiguration, RenderError> {
        let failed = RenderError::Surface;
        let caps = surface.get_capabilities(&self.gpu);
        let format = surface_format(&caps.formats, presentation.format)
            .ok_or_else(|| failed("no surface format".into()))?;
        let opaque = caps.alpha_modes.contains(&CompositeAlphaMode::Opaque);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            desired_maximum_frame_latency: presentation.latency,
            present_mode: present_mode(&caps.present_modes, presentation.vsync),
            alpha_mode: if opaque { CompositeAlphaMode::Opaque } else { CompositeAlphaMode::Auto },
            view_formats: Vec::new(),
        };
        Ok(config)
    }

    pub fn present_layers(
        &self,
        target: &mut Surface,
        (width, height): (u32, u32),
        layers: &[Layer<'_>],
        before_present: impl FnOnce(),
    ) -> Result<bool, RenderError> {
        if target.size() != (width, height) {
            self.check_surface(width, height)?;
            (target.config.width, target.config.height) = (width, height);
            self.scoped(|| target.surface.configure(&self.device, &target.config))?;
        }
        let (frame, got) = match target.surface.get_current_texture() {
            Acquired::Success(frame) => (Some(frame), Got::Fresh),
            Acquired::Suboptimal(frame) => (Some(frame), Got::Stale),
            Acquired::Timeout | Acquired::Occluded => (None, Got::Busy),
            Acquired::Outdated => (None, Got::Outdated),
            Acquired::Lost => return Err(RenderError::Surface("the surface was lost".into())),
            Acquired::Validation => return Err(RenderError::Gpu("cannot acquire a surface texture".into())),
        };
        let (draw, renew) = steps(got);
        let Some(frame) = frame.filter(|_| draw) else {
            if renew {
                self.renew(target)?;
            }
            return Ok(false);
        };
        let view = frame.texture.create_view(&Default::default());
        self.scoped(|| {
            let format = target.config.format;
            let encoder = self.encode(&view, &mut target.slots, layers, (width, height), format);
            self.queue.submit([encoder.finish()]);
        })
        .inspect_err(|_| target.slots.clear())?;
        before_present();
        self.queue.present(frame);
        if renew {
            self.renew(target)?;
        }
        Ok(true)
    }

    fn check_surface(&self, width: u32, height: u32) -> Result<(), RenderError> {
        self.limits.fits(width, height).then_some(()).ok_or(RenderError::InvalidSize(width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::TextureFormat::*;

    #[test]
    fn desktop_presentation_never_waits_for_vsync_when_it_can_avoid_it() {
        assert_eq!(present_mode(&[PresentMode::Fifo, PresentMode::Mailbox], false), PresentMode::Mailbox);
        assert_eq!(present_mode(&[PresentMode::Immediate, PresentMode::Fifo], false), PresentMode::Immediate);
        assert_eq!(present_mode(&[PresentMode::Fifo], false), PresentMode::Fifo);
        assert_eq!(present_mode(&[PresentMode::Mailbox, PresentMode::Fifo], true), PresentMode::Fifo);
    }

    #[test]
    fn a_suboptimal_swapchain_is_drawn_once_then_configured_again() {
        assert_eq!(steps(Got::Fresh), (true, false));
        assert_eq!(steps(Got::Stale), (true, true));
        assert_eq!(steps(Got::Outdated), (false, true));
        assert_eq!(steps(Got::Busy), (false, false));
    }

    #[test]
    fn surface_formats_prefer_the_requested_format_then_the_first_srgb() {
        let desktop = Presentation::DESKTOP.format;
        assert_eq!(surface_format(&[Rgba8UnormSrgb, Bgra8UnormSrgb], desktop), Some(Bgra8UnormSrgb));
        assert_eq!(surface_format(&[Bgra8Unorm, Rgba8UnormSrgb], desktop), Some(Rgba8UnormSrgb));
        assert_eq!(surface_format(&[Rgb10a2Unorm], desktop), Some(Rgb10a2Unorm));
        assert_eq!(surface_format(&[], desktop), None);
        let mobile = Presentation::MOBILE.format;
        assert_eq!(
            surface_format(&[Rgba8Unorm, Rgba8UnormSrgb, Bgra8UnormSrgb], mobile),
            Some(Rgba8UnormSrgb)
        );
    }
}
