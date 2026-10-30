use crate::plane::{PLANES, modifiers};
use crate::texture::{GpuImage, create_texture, gpu_image};
use crate::yuv::{group, uniform};
use crate::{RenderError, Renderer};
use ash::{ext, khr};
use std::os::fd::BorrowedFd;
use wgpu::hal::api::Vulkan;
use wgpu::{TextureFormat as Format, TextureUsages as Usage};

const KEPT: usize = 16;

pub struct Shared<'a> {
    pub size: (u32, u32),
    pub modifier: u64,
    pub planes: [(BorrowedFd<'a>, u32, u32); 2],
    pub bt709: bool,
    pub full: bool,
    pub turn: u8,
    pub fit: (u32, u32),
}

type Key = ((u32, u32), u64, bool, bool, u8, (u32, u32));
type Seen = [(u64, u32); 2];

pub struct Imported {
    key: Key,
    rows: wgpu::Buffer,
    image: GpuImage,
    groups: Vec<(Seen, wgpu::BindGroup, [wgpu::Texture; 2])>,
}

fn turned((width, height): (u32, u32), turn: u8) -> (u32, u32) {
    if turn % 2 == 1 { (height, width) } else { (width, height) }
}

fn fitted(size: (u32, u32), (fw, fh): (u32, u32)) -> (u32, u32) {
    let scale =
        (f64::from(fw.max(1)) / f64::from(size.0)).min(f64::from(fh.max(1)) / f64::from(size.1)).min(1.0);
    (((f64::from(size.0) * scale).round() as u32).max(1), ((f64::from(size.1) * scale).round() as u32).max(1))
}

impl Renderer {
    pub fn importable(&self) -> Vec<u64> {
        let Some(hal) = (unsafe { self.device.as_hal::<Vulkan>() }) else { return Vec::new() };
        let enabled = |name| hal.enabled_device_extensions().contains(&name);
        let needed = [
            ext::external_memory_dma_buf::NAME,
            khr::external_memory_fd::NAME,
            ext::image_drm_format_modifier::NAME,
        ];
        if !needed.into_iter().all(enabled) {
            return Vec::new();
        }
        let (instance, phd) = (hal.shared_instance().raw_instance(), hal.raw_physical_device());
        let [luma, chroma] = PLANES.map(|(format, _)| modifiers(instance, phd, format));
        luma.into_iter().filter(|m| chroma.contains(m)).collect()
    }

    pub fn convert_shared<'a>(
        &self,
        slot: &'a mut Option<Imported>,
        frame: &Shared<'_>,
    ) -> Result<&'a GpuImage, RenderError> {
        let (w, h) = frame.size;
        if !self.limits.fits(w, h) || w < 2 || h < 2 {
            return Err(RenderError::InvalidSize(w, h));
        }
        let key = (frame.size, frame.modifier, frame.bt709, frame.full, frame.turn, frame.fit);
        if slot.as_ref().is_none_or(|imported| imported.key != key) {
            let (rows, size) =
                (uniform(self, frame.bt709, frame.full, frame.turn), turned(frame.size, frame.turn));
            let usage = Usage::TEXTURE_BINDING | Usage::COPY_SRC | Usage::RENDER_ATTACHMENT;
            let target = fitted(size, frame.fit);
            let made = self.scoped(|| {
                create_texture(
                    &self.device,
                    "wprs-render shared video",
                    target,
                    Format::Rgba8UnormSrgb,
                    usage,
                )
            });
            *slot = Some(Imported { key, rows, image: gpu_image(made?), groups: Vec::new() });
        }
        let imported = slot.as_mut().expect("made above");
        let seen =
            frame.planes.map(|(fd, offset, _)| (rustix::fs::fstat(fd).map_or(0, |s| s.st_ino), offset));
        let found = imported.groups.iter().position(|(known, ..)| *known == seen);
        let index = match found {
            Some(index) => index,
            None => {
                let half = (w.div_ceil(2), h.div_ceil(2));
                let luma = unsafe { self.plane(frame, 0, (w, h)) }?;
                let chroma = unsafe { self.plane(frame, 1, half) }?;
                let group = group(self, &imported.rows, [&luma, &chroma]);
                if imported.groups.len() >= KEPT {
                    imported.groups.remove(0);
                }
                imported.groups.push((seen, group, [luma, chroma]));
                imported.groups.len() - 1
            }
        };
        let group = &imported.groups[index].1;
        self.scoped(|| self.convert(group, &imported.image))?;
        Ok(&imported.image)
    }
}
