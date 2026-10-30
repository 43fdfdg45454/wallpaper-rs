use crate::RenderError;
use crate::exported::Exported;
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle};
use ash::{ext, khr};
use std::os::fd::{FromRawFd, OwnedFd};
use wgpu::hal::api::Vulkan;

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

    pub fn allocate(&self, width: u32, height: u32, modifiers: &[u64]) -> Result<Dmabuf, RenderError> {
        let offered = |copied: bool| {
            let found = self.modifiers.iter().copied().filter(|m| modifiers.contains(m));
            found.filter(|m| self.copied.contains(m) == copied).collect::<Vec<u64>>()
        };
        let direct = offered(false);
        let copy = direct.is_empty();
        let wanted = if copy { offered(true) } else { direct };
        let usage =
            if copy { vk::ImageUsageFlags::TRANSFER_DST } else { vk::ImageUsageFlags::COLOR_ATTACHMENT };
        let edge = 1..=self.device.limits().max_texture_dimension_2d;
        if wanted.is_empty() || !edge.contains(&width) || !edge.contains(&height) {
            return Err(gpu(format!("no exportable layout for {width}x{height}")));
        }
        let hal = unsafe { self.device.as_hal::<Vulkan>() }.ok_or_else(|| gpu("not a Vulkan device"))?;
        let raw = hal.raw_device();
        let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(self.handle);
        let mut list = vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(&wanted);
        let tiling =
            self.tiled.as_ref().map_or(vk::ImageTiling::LINEAR, |_| vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT);
        let mut info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(FORMAT)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(tiling)
            .usage(usage)
            .push_next(&mut external);
        if self.tiled.is_some() {
            info = info.push_next(&mut list);
        }
        let image = unsafe { raw.create_image(&info, None) }.map_err(gpu)?;
        let destroy = |memory: Option<vk::DeviceMemory>| unsafe {
            memory.into_iter().for_each(|memory| raw.free_memory(memory, None));
            raw.destroy_image(image, None);
        };
        let mappable = wanted == [LINEAR];
        let (memory, size) = unsafe { self.bind(raw, image, mappable) }.inspect_err(|_| destroy(None))?;
        let (fd, modifier, layout) =
            unsafe { self.plane(raw, image, memory) }.inspect_err(|_| destroy(Some(memory)))?;
        let (start, size) = match self.handle {
            Handle::OPAQUE_FD => mesa_header(&fd).ok_or_else(|| gpu("unknown memory layout")),
            _ => Ok((0, size)),
        }
        .inspect_err(|_| destroy(Some(memory)))?;
        let copies = copy.then(|| self.copies.clone());
        let image = unsafe { Exported::adopt(&self.device, &hal, (image, memory), (width, height), copies) };
        let narrow = |v: u64| u32::try_from(v).map_err(gpu);
        let (offset, stride) = (narrow(start + layout.offset)?, narrow(layout.row_pitch)?);
        Ok(Dmabuf { fd, modifier, offset, stride, size: narrow(size)?, image })
    }

    unsafe fn bind(
        &self,
        raw: &ash::Device,
        image: vk::Image,
        mappable: bool,
    ) -> Result<(vk::DeviceMemory, u64), RenderError> {
        let need = unsafe { raw.get_image_memory_requirements(image) };
        let allowed = |i: &usize| need.memory_type_bits & (1 << i) != 0;
        let (local, seen) = (vk::MemoryPropertyFlags::DEVICE_LOCAL, vk::MemoryPropertyFlags::HOST_VISIBLE);
        let has = |flags: vk::MemoryPropertyFlags| move |i: &usize| self.memory[*i].contains(flags);
        let types = || (0..self.memory.len()).filter(allowed);
        let shared = types().find(has(local | seen)).filter(|_| mappable);
        let index = shared.or_else(|| types().find(has(local))).or_else(|| types().next());
        let index = index.ok_or_else(|| gpu("no memory type"))?;
        let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(self.handle);
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(need.size)
            .memory_type_index(index as u32)
            .push_next(&mut export)
            .push_next(&mut dedicated);
        let memory = unsafe { raw.allocate_memory(&info, None) }.map_err(gpu)?;
        let bound = unsafe { raw.bind_image_memory(image, memory, 0) }.map_err(gpu);
        bound.map(|()| (memory, need.size)).inspect_err(|_| unsafe { raw.free_memory(memory, None) })
    }

    unsafe fn plane(
        &self,
        raw: &ash::Device,
        image: vk::Image,
        memory: vk::DeviceMemory,
    ) -> Result<Plane, RenderError> {
        let get = vk::MemoryGetFdInfoKHR::default().memory(memory).handle_type(self.handle);
        let fd = unsafe { self.fd.get_memory_fd(&get) }.map_err(gpu)?;
        let fd = (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) });
        let fd = fd.ok_or_else(|| gpu("the driver returned no file descriptor"))?;
        let (modifier, aspect) = match &self.tiled {
            Some(tiled) => {
                let mut properties = vk::ImageDrmFormatModifierPropertiesEXT::default();
                unsafe { tiled.get_image_drm_format_modifier_properties(image, &mut properties) }
                    .map_err(gpu)?;
                (properties.drm_format_modifier, vk::ImageAspectFlags::MEMORY_PLANE_0_EXT)
            }
            None => (LINEAR, vk::ImageAspectFlags::COLOR),
        };
        let part = vk::ImageSubresource { aspect_mask: aspect, mip_level: 0, array_layer: 0 };
        Ok((fd, modifier, unsafe { raw.get_image_subresource_layout(image, part) }))
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
