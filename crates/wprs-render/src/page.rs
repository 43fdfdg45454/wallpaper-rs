use crate::plane::modifiers;
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::{RenderError, Renderer};
use ash::{ext, vk};
use std::os::fd::BorrowedFd;
use wgpu::TextureFormat as Format;
use wgpu::hal::api::Vulkan;

const PAGE: (vk::Format, Format) = (vk::Format::B8G8R8A8_SRGB, Format::Bgra8UnormSrgb);
pub const XRGB8888: u32 = 0x3432_5258;
pub const ARGB8888: u32 = 0x3432_5241;

pub struct PageBuffer<'a> {
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
    pub modifier: u64,
    pub fourcc: u32,
    pub size: (u32, u32),
}

impl Renderer {
    pub fn page_modifiers(&self) -> Vec<u64> {
        let Some(hal) = (unsafe { self.device.as_hal::<Vulkan>() }) else { return Vec::new() };
        let needed = [ext::external_memory_dma_buf::NAME, ext::image_drm_format_modifier::NAME];
        if !needed.into_iter().all(|name| hal.enabled_device_extensions().contains(&name)) {
            return Vec::new();
        }
        modifiers(hal.shared_instance().raw_instance(), hal.raw_physical_device(), PAGE.0)
    }

    pub fn render_node(&self) -> Option<String> {
        let hal = unsafe { self.device.as_hal::<Vulkan>() }?;
        let (instance, phd) = (hal.shared_instance().raw_instance(), hal.raw_physical_device());
        let extensions = unsafe { instance.enumerate_device_extension_properties(phd) }.ok()?;
        let named =
            |e: &vk::ExtensionProperties| e.extension_name_as_c_str() == Ok(ext::physical_device_drm::NAME);
        extensions.iter().any(named).then_some(())?;
        let mut drm = vk::PhysicalDeviceDrmPropertiesEXT::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut drm);
        unsafe { instance.get_physical_device_properties2(phd, &mut properties) };
        (drm.has_render == vk::TRUE).then(|| format!("/dev/dri/renderD{}", drm.render_minor))
    }

    pub fn import_page(&self, buffer: &PageBuffer<'_>) -> Result<GpuImage, RenderError> {
        let (w, h) = buffer.size;
        if !self.limits.fits(w, h) || ![XRGB8888, ARGB8888].contains(&buffer.fourcc) {
            return Err(RenderError::InvalidSize(w, h));
        }
        let plane = (buffer.fd, buffer.offset, buffer.stride, buffer.modifier);
        let texture = unsafe { self.import_plane(plane, buffer.size, PAGE) }?;
        Ok(gpu_image(texture))
    }

    pub fn upload_page(
        &self,
        image: &mut Option<GpuImage>,
        bgra: &[u8],
        (width, height): (u32, u32),
        stride: u32,
    ) -> Result<(), RenderError> {
        let needed = u64::from(stride) * u64::from(height.saturating_sub(1)) + u64::from(width) * 4;
        let fits = self.limits.fits(width, height) && stride >= width * 4 && bgra.len() as u64 >= needed;
        fits.then_some(()).ok_or(RenderError::InvalidSize(width, height))?;
        if image.as_ref().is_none_or(|i| i.size() != (width, height)) {
            let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
            let made = self.scoped(|| {
                create_texture(&self.device, "wprs-render page", (width, height), PAGE.1, usage)
            })?;
            *image = Some(gpu_image(made));
        }
        let texture = &image.as_ref().expect("made above").texture;
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        };
        self.queue.write_texture(texture.as_image_copy(), bgra, layout, texture.size());
        Ok(())
    }
}
