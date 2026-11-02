use crate::bind::Slot;
use crate::draw::TARGET_FORMAT;
use crate::{Layer, RenderError, Renderer};
use wgpu::{CompositeAlphaMode, CurrentSurfaceTexture as Acquired, PresentMode, TextureFormat as Format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    pub vsync: bool,
    pub latency: u32,
    pub format: Option<Format>,
    pub hdr: bool,
}

impl Presentation {
    pub const DESKTOP: Self = Self { vsync: false, latency: 2, format: Some(TARGET_FORMAT), hdr: false };
    pub const MOBILE: Self = Self { vsync: true, latency: 1, format: None, hdr: false };
}

pub struct Surface {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) presentation: Presentation,
    pub(crate) slots: Vec<Slot>,
    pub(crate) offered: bool,
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
        let (config, offered) = self.surface_config(&surface, (width, height), presentation)?;
        self.scoped(|| surface.configure(&self.device, &config))?;
        Ok(Surface { surface, config, presentation, slots: Vec::new(), offered })
    }

    pub(crate) fn renew(&self, target: &mut Surface) -> Result<(), RenderError> {
        let size = (target.config.width, target.config.height);
        let (config, offered) = self.surface_config(&target.surface, size, target.presentation)?;
        target.offered = offered;
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
    ) -> Result<(wgpu::SurfaceConfiguration, bool), RenderError> {
        let failed = RenderError::Surface;
        let caps = surface.get_capabilities(&self.gpu);
        let format = surface_format(&caps.formats, presentation.format)
            .ok_or_else(|| failed("no surface format".into()))?;
        let (offered, lit) = crate::hdr::lit(&caps, presentation.hdr);
        let opaque = caps.alpha_modes.contains(&CompositeAlphaMode::Opaque);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: lit.map_or(format, |l| l.0),
            color_space: lit.map_or(wgpu::SurfaceColorSpace::Auto, |l| l.1),
            width,
            height,
            desired_maximum_frame_latency: presentation.latency,
            present_mode: present_mode(&caps.present_modes, presentation.vsync),
            alpha_mode: if opaque { CompositeAlphaMode::Opaque } else { CompositeAlphaMode::Auto },
            view_formats: Vec::new(),
        };
        Ok((config, offered))
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
            let encoder = self.encode(&view, &mut target.slots, layers, (width, height), format, None);
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
mod tests;
