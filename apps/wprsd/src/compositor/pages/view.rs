use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};
use wprs_render::{GpuImage, PageBuffer, RenderError, Renderer};

struct Mapped {
    map: *mut u8,
    len: usize,
    stride: u32,
    size: (u32, u32),
}

impl Drop for Mapped {
    fn drop(&mut self) {
        let _ = unsafe { munmap(self.map.cast(), self.len) };
    }
}

#[derive(Default)]
pub(in crate::compositor) struct ViewSlot {
    pub(in crate::compositor) id: u32,
    pub(in crate::compositor) size: (u32, u32),
    pub(in crate::compositor) opened: bool,
    pub(in crate::compositor) visible: Option<bool>,
    pub(in crate::compositor) live: bool,
    pub(in crate::compositor) unseen: Option<std::time::Instant>,
    pub(in crate::compositor) spec: Option<crate::pages::PageSpec>,
    pub(in crate::compositor) sent: serde_json::Map<String, serde_json::Value>,
    pub(in crate::compositor) fps: u32,
    pub(in crate::compositor) texture: Option<GpuImage>,
    pub(in crate::compositor) shown: Option<u32>,
    pub(in crate::compositor) fresh: bool,
    pub(in crate::compositor) pointer: Option<(f64, f64)>,
    pub(in crate::compositor) pressed: [Option<std::time::Instant>; 2],
    imported: HashMap<u32, GpuImage>,
    ring: Option<Mapped>,
    copied: Option<GpuImage>,
}

unsafe impl Send for ViewSlot {}

pub(super) struct Shared {
    pub(in crate::compositor) id: u32,
    pub(in crate::compositor) size: (u32, u32),
    pub(in crate::compositor) fourcc: u32,
    pub(in crate::compositor) modifier: u64,
    pub(in crate::compositor) offset: u32,
    pub(in crate::compositor) stride: u32,
}

impl ViewSlot {
    pub(super) fn new(id: u32, size: (u32, u32)) -> ViewSlot {
        ViewSlot { id, size, ..ViewSlot::default() }
    }

    pub(super) fn reset(&mut self) {
        (self.opened, self.visible, self.fps, self.shown, self.pointer) = (false, None, 0, None, None);
        self.imported.clear();
        self.ring = None;
    }

    pub(super) fn forget(&mut self) {
        let shown = self.shown.and_then(|id| self.imported.remove(&id).map(|image| (id, image)));
        self.imported.clear();
        self.imported.extend(shown);
    }

    pub(super) fn import(
        &mut self,
        renderer: &Renderer,
        b: &Shared,
        fd: &OwnedFd,
    ) -> Result<(), RenderError> {
        let (fd, offset, stride, modifier, fourcc, size) =
            (fd.as_fd(), b.offset, b.stride, b.modifier, b.fourcc, b.size);
        let image = renderer.import_page(&PageBuffer { fd, offset, stride, modifier, fourcc, size })?;
        self.imported.insert(b.id, image);
        Ok(())
    }

    pub(super) fn frame(&mut self, id: u32) -> Option<u32> {
        let image = self.imported.get(&id)?.clone();
        self.texture = Some(image);
        self.fresh = true;
        self.shown.replace(id).filter(|old| *old != id)
    }

    pub(super) fn map(&mut self, fd: &OwnedFd, size: (u32, u32), stride: u32) -> std::io::Result<()> {
        let len = stride as usize * size.1 as usize * 2;
        let sealed = rustix::fs::fcntl_get_seals(fd)?.contains(rustix::fs::SealFlags::SHRINK);
        if !sealed || (rustix::fs::fstat(fd)?.st_size as u64) < len as u64 {
            return Err(std::io::Error::other("its frames are not in a sealed buffer of their size"));
        }
        let map = unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ, MapFlags::SHARED, fd, 0) }?;
        self.ring = Some(Mapped { map: map.cast(), len, stride, size });
        Ok(())
    }

    pub(super) fn pixels(
        &mut self,
        renderer: &Renderer,
        slot: u32,
        size: (u32, u32),
    ) -> Result<(), RenderError> {
        let Some(ring) = self.ring.as_ref().filter(|r| r.size == size && slot < 2) else {
            return Err(RenderError::InvalidSize(size.0, size.1));
        };
        let room = ring.len / 2;
        let data = unsafe { std::slice::from_raw_parts(ring.map.add(slot as usize * room), room) };
        renderer.upload_page(&mut self.copied, data, size, ring.stride)?;
        (self.texture, self.fresh, self.shown) = (self.copied.clone(), true, None);
        Ok(())
    }
}
