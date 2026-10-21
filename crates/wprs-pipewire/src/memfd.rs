//! Buffers backed by one sealed memfd each, allocated by the stream itself.
//!
//! Consumers such as KPipeWire map a MemFd buffer from offset 0 and ignore
//! `mapoffset`, and crash on MemPtr buffers. Buffers allocated by PipeWire
//! share one memfd at different offsets (or arrive as MemPtr), so the stream
//! allocates its own: one memfd per buffer, mapped from offset 0, as KWin
//! does for its screen casts.

use pipewire::spa::sys as spa_sys;
use pipewire::sys as pw_sys;
use rustix::fs::{MemfdFlags, SealFlags};
use rustix::mm::{MapFlags, ProtFlags};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};

/// Fills the first data plane of `buffer` with a new memfd of `size` bytes.
///
/// # Safety
/// `buffer` must be a buffer handed to the stream's `add_buffer` callback.
pub unsafe fn add(buffer: *mut pw_sys::pw_buffer, size: u32) -> rustix::io::Result<()> {
    // SAFETY: the caller guarantees `buffer` and its spa_buffer are valid.
    let data = unsafe {
        let spa_buffer = (*buffer).buffer;
        if spa_buffer.is_null() || (*spa_buffer).n_datas < 1 {
            return Err(rustix::io::Errno::INVAL);
        }
        &mut *(*spa_buffer).datas
    };

    let fd = rustix::fs::memfd_create("wallpaper-rs", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
    rustix::fs::ftruncate(&fd, u64::from(size))?;
    rustix::fs::fcntl_add_seals(&fd, SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL)?;
    // SAFETY: a fresh shared mapping of a file we own; unmapped in `remove`.
    let ptr = unsafe {
        rustix::mm::mmap(
            std::ptr::null_mut(),
            size as usize,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::SHARED,
            &fd,
            0,
        )?
    };

    data.type_ = spa_sys::SPA_DATA_MemFd;
    data.flags = spa_sys::SPA_DATA_FLAG_READWRITE | spa_sys::SPA_DATA_FLAG_MAPPABLE;
    data.fd = i64::from(fd.into_raw_fd());
    data.mapoffset = 0;
    data.maxsize = size;
    data.data = ptr;
    Ok(())
}

/// Releases a buffer filled by [`add`].
///
/// # Safety
/// `buffer` must be a buffer handed to the stream's `remove_buffer` callback.
pub unsafe fn remove(buffer: *mut pw_sys::pw_buffer) {
    // SAFETY: the caller guarantees `buffer` and its spa_buffer are valid.
    unsafe {
        let spa_buffer = (*buffer).buffer;
        if spa_buffer.is_null() || (*spa_buffer).n_datas < 1 {
            return;
        }
        let data = &mut *(*spa_buffer).datas;
        if data.type_ != spa_sys::SPA_DATA_MemFd || data.fd < 0 {
            return;
        }
        if !data.data.is_null() {
            let _ = rustix::mm::munmap(data.data, data.maxsize as usize);
        }
        drop(OwnedFd::from_raw_fd(data.fd as i32));
        data.data = std::ptr::null_mut();
        data.fd = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_and_releases_a_mapped_memfd() {
        let mut chunk: spa_sys::spa_chunk = unsafe { std::mem::zeroed() };
        let mut data: spa_sys::spa_data = unsafe { std::mem::zeroed() };
        data.chunk = &mut chunk;
        let mut spa_buffer: spa_sys::spa_buffer = unsafe { std::mem::zeroed() };
        spa_buffer.n_datas = 1;
        spa_buffer.datas = &mut data;
        let mut buffer: pw_sys::pw_buffer = unsafe { std::mem::zeroed() };
        buffer.buffer = &mut spa_buffer;

        unsafe { add(&mut buffer, 4096) }.unwrap();
        assert_eq!(data.type_, spa_sys::SPA_DATA_MemFd);
        assert_eq!((data.mapoffset, data.maxsize), (0, 4096));
        assert!(data.fd >= 0 && !data.data.is_null());

        // Writes through the mapping are visible through the fd from offset
        // 0, which is how consumers read it.
        unsafe { std::slice::from_raw_parts_mut(data.data.cast::<u8>(), 4096)[..4].copy_from_slice(b"wprs") };
        let mut head = [0u8; 4];
        rustix::io::pread(
            unsafe { std::os::fd::BorrowedFd::borrow_raw(data.fd as i32) },
            &mut head,
            0,
        )
        .unwrap();
        assert_eq!(&head, b"wprs");

        unsafe { remove(&mut buffer) };
        assert_eq!(data.fd, -1);
        assert!(data.data.is_null());
    }
}
