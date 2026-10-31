use super::{Dmabuf, Dmabufs, FORMAT, LINEAR, Plane, gpu, mesa_header};
use crate::RenderError;
use crate::exported::Exported;
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle};
use std::os::fd::{FromRawFd, OwnedFd};
use wgpu::hal::api::Vulkan;

impl Dmabufs {
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
