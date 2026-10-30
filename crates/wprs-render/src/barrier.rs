use crate::Renderer;
use ash::vk::{self, ImageLayout, PipelineStageFlags as Stage};
use wgpu::hal::api::Vulkan;

impl Renderer {
    pub(crate) fn barrier(
        &self,
        image: vk::Image,
        release: bool,
        copy: bool,
        kept: bool,
    ) -> wgpu::CommandBuffer {
        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);
        let (drawn, layout, stage) = match copy {
            true => (vk::AccessFlags::TRANSFER_WRITE, ImageLayout::TRANSFER_DST_OPTIMAL, Stage::TRANSFER),
            false => (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                Stage::COLOR_ATTACHMENT_OUTPUT,
            ),
        };
        let ignored = vk::QUEUE_FAMILY_IGNORED;
        let read = if copy { vk::AccessFlags::empty() } else { vk::AccessFlags::COLOR_ATTACHMENT_READ };
        let barrier = vk::ImageMemoryBarrier::default().image(image).subresource_range(range);
        let device = unsafe { self.device.as_hal::<Vulkan>() };
        let record = |encoder: Option<&mut wgpu::hal::vulkan::CommandEncoder>| {
            let (Some(device), Some(encoder)) = (device.as_deref(), encoder) else { return };
            let (barrier, from, to) = match release {
                true => (
                    barrier
                        .src_access_mask(drawn)
                        .old_layout(layout)
                        .new_layout(ImageLayout::GENERAL)
                        .src_queue_family_index(device.queue_family_index())
                        .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL),
                    stage,
                    Stage::BOTTOM_OF_PIPE,
                ),
                false if kept => (
                    barrier
                        .dst_access_mask(drawn | read)
                        .old_layout(ImageLayout::GENERAL)
                        .new_layout(layout)
                        .src_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
                        .dst_queue_family_index(device.queue_family_index()),
                    Stage::TOP_OF_PIPE,
                    stage,
                ),
                false => (
                    barrier
                        .dst_access_mask(drawn)
                        .new_layout(layout)
                        .src_queue_family_index(ignored)
                        .dst_queue_family_index(ignored),
                    Stage::TOP_OF_PIPE,
                    stage,
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
