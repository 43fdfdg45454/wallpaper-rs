use crate::bind::Slot;
use crate::dmabuf::Dmabufs;
use crate::draw::TARGET_FORMAT;
use crate::readback::GPU_TIMEOUT;
use crate::{Layer, RenderError, Renderer};
use ash::vk::{self, ExternalMemoryHandleTypeFlags as Handle, ImageLayout, PipelineStageFlags as Stage};
use std::sync::{Mutex, PoisonError};
use wgpu::hal::api::Vulkan;
use wgpu::wgt::TextureUses;

pub struct Exported {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    raw: vk::Image,
    slots: Mutex<Vec<Slot>>,
}

impl Exported {
    pub(crate) unsafe fn adopt(
        device: &wgpu::Device,
        hal: &wgpu::hal::vulkan::Device,
        raw: vk::Image,
        memory: vk::DeviceMemory,
        (width, height): (u32, u32),
    ) -> Self {
        let (label, size) =
            (Some("wprs-render dmabuf"), wgpu::Extent3d { width, height, depth_or_array_layers: 1 });
        let (dimension, format) = (wgpu::TextureDimension::D2, TARGET_FORMAT);
        let (memory_flags, usage) = (wgpu::hal::MemoryFlags::empty(), TextureUses::COLOR_TARGET);
        let described = wgpu::hal::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            memory_flags,
            view_formats: Vec::new(),
        };
        let dedicated = wgpu::hal::vulkan::TextureMemory::Dedicated(memory);
        let texture = unsafe { hal.texture_from_raw(raw, &described, None, dedicated) };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let desc = wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        };
        let texture =
            unsafe { device.create_texture_from_hal::<Vulkan>(texture, &desc, TextureUses::UNINITIALIZED) };
        Self { view: texture.create_view(&Default::default()), texture, raw, slots: Mutex::default() }
    }
}

impl Renderer {
    pub fn dmabufs(&self, emulate: bool) -> Option<Dmabufs> {
        Dmabufs::new(&self.device, if emulate { Handle::OPAQUE_FD } else { Handle::DMA_BUF_EXT })
    }

    pub fn render_exported(&self, image: &Exported, layers: &[Layer<'_>]) -> Result<(), RenderError> {
        let mut slots = image.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let size = (image.texture.width(), image.texture.height());
        let done = self.scoped(|| {
            let draw = self.encode(&image.view, &mut slots, layers, size, TARGET_FORMAT);
            let [acquire, release] = [false, true].map(|release| self.barrier(image.raw, release));
            self.queue.submit([acquire, draw.finish(), release])
        });
        let done = done.inspect_err(|_| slots.clear())?;
        let wait = wgpu::PollType::Wait { submission_index: Some(done), timeout: Some(GPU_TIMEOUT) };
        self.device.poll(wait).map(drop).map_err(|e| RenderError::DeviceLost(e.to_string()))
    }

    fn barrier(&self, image: vk::Image, release: bool) -> wgpu::CommandBuffer {
        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);
        let (drawn, ignored) = (vk::AccessFlags::COLOR_ATTACHMENT_WRITE, vk::QUEUE_FAMILY_IGNORED);
        let barrier = vk::ImageMemoryBarrier::default().image(image).subresource_range(range);
        let device = unsafe { self.device.as_hal::<Vulkan>() };
        let record = |encoder: Option<&mut wgpu::hal::vulkan::CommandEncoder>| {
            let (Some(device), Some(encoder)) = (device.as_deref(), encoder) else { return };
            let (barrier, from, to) = match release {
                true => (
                    barrier
                        .src_access_mask(drawn)
                        .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(ImageLayout::GENERAL)
                        .src_queue_family_index(device.queue_family_index())
                        .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL),
                    Stage::COLOR_ATTACHMENT_OUTPUT,
                    Stage::BOTTOM_OF_PIPE,
                ),
                false => (
                    barrier
                        .dst_access_mask(drawn)
                        .new_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .src_queue_family_index(ignored)
                        .dst_queue_family_index(ignored),
                    Stage::TOP_OF_PIPE,
                    Stage::COLOR_ATTACHMENT_OUTPUT,
                ),
            };
            let (buffer, flags) = (unsafe { encoder.raw_handle() }, vk::DependencyFlags::empty());
            unsafe {
                device.raw_device().cmd_pipeline_barrier(buffer, from, to, flags, &[], &[], &[barrier])
            };
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        unsafe { encoder.as_hal_mut::<Vulkan, _, _>(record) };
        encoder.finish()
    }
}
