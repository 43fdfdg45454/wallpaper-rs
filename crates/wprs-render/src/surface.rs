use crate::bind::Slot;
use crate::draw::TARGET_FORMAT;
use crate::shader::pipelines;
use crate::{Layer, RenderError, Renderer};
use wgpu::{CompositeAlphaMode, CurrentSurfaceTexture as Acquired, PresentMode};

pub struct Surface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipelines: Option<[wgpu::RenderPipeline; 2]>,
    slots: Vec<Slot>,
}

impl Surface {
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn present_mode(&self) -> PresentMode {
        self.config.present_mode
    }
}

fn present_mode(offered: &[PresentMode]) -> PresentMode {
    let preferred = [PresentMode::Mailbox, PresentMode::Immediate];
    preferred.into_iter().find(|mode| offered.contains(mode)).unwrap_or(PresentMode::Fifo)
}

fn surface_format(offered: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    let srgb = || offered.iter().copied().find(wgpu::TextureFormat::is_srgb);
    offered.contains(&TARGET_FORMAT).then_some(TARGET_FORMAT).or_else(srgb).or(offered.first().copied())
}

impl Renderer {
    pub fn create_surface(
        &self,
        window: impl wgpu::DisplayAndWindowHandle + 'static,
        (width, height): (u32, u32),
    ) -> Result<Surface, RenderError> {
        self.check_surface(width, height)?;
        let failed = RenderError::Surface;
        let surface = self.instance.create_surface(window).map_err(|e| failed(e.to_string()))?;
        if !self.gpu.is_surface_supported(&surface) {
            return Err(failed(format!("{} cannot present to this surface", self.adapter())));
        }
        let caps = surface.get_capabilities(&self.gpu);
        let format = surface_format(&caps.formats).ok_or_else(|| failed("no surface format".into()))?;
        let opaque = caps.alpha_modes.contains(&CompositeAlphaMode::Opaque);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            desired_maximum_frame_latency: 2,
            present_mode: present_mode(&caps.present_modes),
            alpha_mode: if opaque { CompositeAlphaMode::Opaque } else { CompositeAlphaMode::Auto },
            view_formats: Vec::new(),
        };
        let pipelines = (format != TARGET_FORMAT).then(|| pipelines(&self.device, &self.layout, format));
        self.scoped(|| surface.configure(&self.device, &config))?;
        Ok(Surface { surface, config, pipelines, slots: Vec::new() })
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
        let frame = match target.surface.get_current_texture() {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => frame,
            Acquired::Timeout | Acquired::Occluded => return Ok(false),
            Acquired::Outdated => {
                self.scoped(|| target.surface.configure(&self.device, &target.config))?;
                return Ok(false);
            }
            Acquired::Lost => return Err(RenderError::Surface("the surface was lost".into())),
            Acquired::Validation => return Err(RenderError::Gpu("cannot acquire a surface texture".into())),
        };
        let view = frame.texture.create_view(&Default::default());
        let pipelines = target.pipelines.as_ref().unwrap_or(&self.pipelines);
        self.scoped(|| {
            let pipelines = (pipelines, target.config.format);
            let encoder = self.encode(&view, &mut target.slots, layers, (width, height), pipelines);
            self.queue.submit([encoder.finish()]);
        })
        .inspect_err(|_| target.slots.clear())?;
        before_present();
        self.queue.present(frame);
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
    fn presentation_never_waits_for_vsync_when_it_can_avoid_it() {
        assert_eq!(present_mode(&[PresentMode::Fifo, PresentMode::Mailbox]), PresentMode::Mailbox);
        assert_eq!(present_mode(&[PresentMode::Immediate, PresentMode::Fifo]), PresentMode::Immediate);
        assert_eq!(present_mode(&[PresentMode::Fifo]), PresentMode::Fifo);
    }

    #[test]
    fn surface_formats_prefer_the_offscreen_format_then_srgb() {
        assert_eq!(surface_format(&[Rgba8UnormSrgb, Bgra8UnormSrgb]), Some(Bgra8UnormSrgb));
        assert_eq!(surface_format(&[Bgra8Unorm, Rgba8UnormSrgb]), Some(Rgba8UnormSrgb));
        assert_eq!(surface_format(&[Rgb10a2Unorm]), Some(Rgb10a2Unorm));
        assert_eq!(surface_format(&[]), None);
    }
}
