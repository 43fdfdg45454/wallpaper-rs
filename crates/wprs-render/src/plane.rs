use crate::import::Shared;
use crate::{RenderError, Renderer};
use ash::khr;
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle};
use std::os::fd::{AsRawFd, IntoRawFd};
use wgpu::TextureFormat as Format;
use wgpu::hal::api::Vulkan;
use wgpu::wgt::TextureUses;

pub(crate) const PLANES: [(vk::Format, Format); 2] =
    [(vk::Format::R8_UNORM, Format::R8Unorm), (vk::Format::R8G8_UNORM, Format::Rg8Unorm)];

fn gpu(e: impl std::fmt::Display) -> RenderError {
    RenderError::Gpu(format!("cannot import a video frame: {e}"))
}

impl Renderer {
    pub(crate) unsafe fn plane(
        &self,
        frame: &Shared<'_>,
        plane: usize,
        (width, height): (u32, u32),
    ) -> Result<wgpu::Texture, RenderError> {
        let hal = unsafe { self.device.as_hal::<Vulkan>() }.ok_or_else(|| gpu("not a Vulkan device"))?;
        let (raw, instance) = (hal.raw_device(), hal.shared_instance().raw_instance());
        let ((format, wanted), (fd, offset, stride)) = (PLANES[plane], frame.planes[plane]);
        let layout =
            [vk::SubresourceLayout { offset: offset.into(), row_pitch: stride.into(), ..Default::default() }];
        let mut explicit = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
            .drm_format_modifier(frame.modifier)
            .plane_layouts(&layout);
        let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(Handle::DMA_BUF_EXT);
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .usage(vk::ImageUsageFlags::SAMPLED)
            .push_next(&mut external)
            .push_next(&mut explicit);
        let image = unsafe { raw.create_image(&info, None) }.map_err(gpu)?;
        let owned = rustix::io::fcntl_dupfd_cloexec(fd, 3).map_err(gpu);
        let memory = owned.and_then(|owned| unsafe { bind(raw, instance, image, owned) });
        let memory = memory.inspect_err(|_| unsafe { raw.destroy_image(image, None) })?;
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let (label, dimension) = (Some("wprs-render shared plane"), wgpu::TextureDimension::D2);
        let described = wgpu::hal::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format: wanted,
            usage: TextureUses::RESOURCE,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: Vec::new(),
        };
        let dedicated = wgpu::hal::vulkan::TextureMemory::Dedicated(memory);
        let texture = unsafe { hal.texture_from_raw(image, &described, None, dedicated) };
        let desc = wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format: wanted,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        Ok(unsafe { self.device.create_texture_from_hal::<Vulkan>(texture, &desc, TextureUses::RESOURCE) })
    }
}

unsafe fn bind(
    raw: &ash::Device,
    instance: &ash::Instance,
    image: vk::Image,
    owned: std::os::fd::OwnedFd,
) -> Result<vk::DeviceMemory, RenderError> {
    let fds = khr::external_memory_fd::Device::new(instance, raw);
    let mut properties = vk::MemoryFdPropertiesKHR::default();
    unsafe { fds.get_memory_fd_properties(Handle::DMA_BUF_EXT, owned.as_raw_fd(), &mut properties) }
        .map_err(gpu)?;
    let need = unsafe { raw.get_image_memory_requirements(image) };
    let types = need.memory_type_bits & properties.memory_type_bits;
    (types != 0).then_some(()).ok_or_else(|| gpu("no memory type"))?;
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut import =
        vk::ImportMemoryFdInfoKHR::default().handle_type(Handle::DMA_BUF_EXT).fd(owned.as_raw_fd());
    let info = vk::MemoryAllocateInfo::default()
        .allocation_size(need.size)
        .memory_type_index(types.trailing_zeros())
        .push_next(&mut import)
        .push_next(&mut dedicated);
    let memory = unsafe { raw.allocate_memory(&info, None) }.map_err(gpu)?;
    let _ = owned.into_raw_fd();
    unsafe { raw.bind_image_memory(image, memory, 0) }
        .map_err(gpu)
        .inspect_err(|_| unsafe { raw.free_memory(memory, None) })?;
    Ok(memory)
}

pub(crate) fn modifiers(instance: &ash::Instance, phd: vk::PhysicalDevice, format: vk::Format) -> Vec<u64> {
    let query = |list: &mut vk::DrmFormatModifierPropertiesListEXT| unsafe {
        let mut properties = vk::FormatProperties2::default().push_next(list);
        instance.get_physical_device_format_properties2(phd, format, &mut properties)
    };
    let mut count = vk::DrmFormatModifierPropertiesListEXT::default();
    query(&mut count);
    let mut all =
        vec![vk::DrmFormatModifierPropertiesEXT::default(); count.drm_format_modifier_count as usize];
    query(&mut vk::DrmFormatModifierPropertiesListEXT::default().drm_format_modifier_properties(&mut all));
    let sampled = |m: &&vk::DrmFormatModifierPropertiesEXT| {
        let features = m.drm_format_modifier_tiling_features;
        m.drm_format_modifier_plane_count == 1 && features.contains(vk::FormatFeatureFlags::SAMPLED_IMAGE)
    };
    let usable = all.iter().filter(sampled).map(|m| m.drm_format_modifier);
    usable.filter(|m| importable(instance, phd, format, *m)).collect()
}

fn importable(instance: &ash::Instance, phd: vk::PhysicalDevice, format: vk::Format, modifier: u64) -> bool {
    let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(Handle::DMA_BUF_EXT);
    let mut drm = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .drm_format_modifier(modifier)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(vk::ImageUsageFlags::SAMPLED)
        .push_next(&mut external)
        .push_next(&mut drm);
    let mut properties = vk::ExternalImageFormatProperties::default();
    let mut out = vk::ImageFormatProperties2::default().push_next(&mut properties);
    let supported = unsafe { instance.get_physical_device_image_format_properties2(phd, &info, &mut out) };
    let features = properties.external_memory_properties.external_memory_features;
    supported.is_ok() && features.contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
}
