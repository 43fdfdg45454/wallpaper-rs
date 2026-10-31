use crate::RenderError;
use crate::exported::Exported;
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle};
use ash::{ext, khr};
use std::os::fd::OwnedFd;
use wgpu::hal::api::Vulkan;

mod alloc;

const FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;
const LINEAR: u64 = 0;

#[derive(Clone)]
pub struct Dmabufs {
    device: wgpu::Device,
    handle: Handle,
    modifiers: Vec<u64>,
    copied: Vec<u64>,
    memory: Vec<vk::MemoryPropertyFlags>,
    fd: khr::external_memory_fd::Device,
    tiled: Option<ext::image_drm_format_modifier::Device>,
    copies: crate::exported::Copies,
}

pub struct Dmabuf {
    pub fd: OwnedFd,
    pub modifier: u64,
    pub offset: u32,
    pub stride: u32,
    pub size: u32,
    pub image: Exported,
}

type Plane = (OwnedFd, u64, vk::SubresourceLayout);

fn gpu(e: impl std::fmt::Display) -> RenderError {
    RenderError::Gpu(format!("cannot export a frame buffer: {e}"))
}

impl Dmabufs {
    pub(crate) fn new(device: &wgpu::Device, handle: Handle) -> Option<Self> {
        let hal = unsafe { device.as_hal::<Vulkan>() }?;
        let enabled = |name| hal.enabled_device_extensions().contains(&name);
        let dmabuf = handle != Handle::DMA_BUF_EXT || enabled(ext::external_memory_dma_buf::NAME);
        if !dmabuf || !enabled(khr::external_memory_fd::NAME) {
            return None;
        }
        let (instance, phd, raw) =
            (hal.shared_instance().raw_instance(), hal.raw_physical_device(), hal.raw_device());
        let tiled = enabled(ext::image_drm_format_modifier::NAME)
            .then(|| ext::image_drm_format_modifier::Device::new(instance, raw));
        let candidates = match tiled {
            Some(_) => format_modifiers(instance, phd),
            None => vec![(LINEAR, vk::FormatFeatureFlags::COLOR_ATTACHMENT)],
        };
        let exports = |m: u64, usage| exportable(instance, phd, handle, tiled.as_ref().map(|_| m), usage);
        let can = |features: vk::FormatFeatureFlags, needed| features.contains(needed);
        let (draw, copy) = (vk::FormatFeatureFlags::COLOR_ATTACHMENT, vk::FormatFeatureFlags::TRANSFER_DST);
        let direct = |&(m, f): &(u64, _)| can(f, draw) && exports(m, vk::ImageUsageFlags::COLOR_ATTACHMENT);
        let copied = |&(m, f): &(u64, _)| can(f, copy) && exports(m, vk::ImageUsageFlags::TRANSFER_DST);
        let mut modifiers: Vec<u64> = candidates.iter().filter(|c| direct(c)).map(|c| c.0).collect();
        let copied: Vec<u64> =
            candidates.iter().filter(|c| !modifiers.contains(&c.0) && copied(c)).map(|c| c.0).collect();
        modifiers.extend(&copied);
        let properties = unsafe { instance.get_physical_device_memory_properties(phd) };
        let memory = properties.memory_types_as_slice().iter().map(|t| t.property_flags).collect();
        let fd = khr::external_memory_fd::Device::new(instance, raw);
        let device = device.clone();
        let copies = Default::default();
        (!modifiers.is_empty()).then(|| Self { device, handle, modifiers, copied, memory, fd, tiled, copies })
    }

    pub fn modifiers(&self) -> &[u64] {
        &self.modifiers
    }
}

fn mesa_header(fd: &OwnedFd) -> Option<(u64, u64)> {
    let mut header = [0u8; 16];
    rustix::io::pread(fd, &mut header, 0).ok().filter(|&n| n == header.len())?;
    let [size, start] = header.as_chunks::<8>().0.try_into().ok()?;
    let [size, start] = [size, start].map(u64::from_ne_bytes);
    let total = rustix::fs::fstat(fd).ok()?.st_size as u64;
    (size == total && start < size).then_some((start, size))
}

fn format_modifiers(instance: &ash::Instance, phd: vk::PhysicalDevice) -> Vec<(u64, vk::FormatFeatureFlags)> {
    let query = |list: &mut vk::DrmFormatModifierPropertiesListEXT| unsafe {
        let mut properties = vk::FormatProperties2::default().push_next(list);
        instance.get_physical_device_format_properties2(phd, FORMAT, &mut properties)
    };
    let mut count = vk::DrmFormatModifierPropertiesListEXT::default();
    query(&mut count);
    let mut all =
        vec![vk::DrmFormatModifierPropertiesEXT::default(); count.drm_format_modifier_count as usize];
    query(&mut vk::DrmFormatModifierPropertiesListEXT::default().drm_format_modifier_properties(&mut all));
    let single = |m: &&vk::DrmFormatModifierPropertiesEXT| m.drm_format_modifier_plane_count == 1;
    all.iter()
        .filter(single)
        .map(|m| (m.drm_format_modifier, m.drm_format_modifier_tiling_features))
        .collect()
}

fn exportable(
    instance: &ash::Instance,
    phd: vk::PhysicalDevice,
    handle: Handle,
    modifier: Option<u64>,
    usage: vk::ImageUsageFlags,
) -> bool {
    let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(handle);
    let mut drm = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .drm_format_modifier(modifier.unwrap_or(LINEAR))
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let tiling = modifier.map_or(vk::ImageTiling::LINEAR, |_| vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT);
    let mut info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(tiling)
        .usage(usage)
        .push_next(&mut external);
    if modifier.is_some() {
        info = info.push_next(&mut drm);
    }
    let mut properties = vk::ExternalImageFormatProperties::default();
    let mut out = vk::ImageFormatProperties2::default().push_next(&mut properties);
    let supported = unsafe { instance.get_physical_device_image_format_properties2(phd, &info, &mut out) };
    let features = properties.external_memory_properties.external_memory_features;
    supported.is_ok() && features.contains(vk::ExternalMemoryFeatureFlags::EXPORTABLE)
}
