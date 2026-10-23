use pipewire::spa::sys as spa_sys;
use pipewire::sys as pw_sys;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};

pub unsafe fn first_data<'a>(buffer: *mut pw_sys::pw_buffer) -> Option<&'a mut spa_sys::spa_data> {
    unsafe {
        let spa_buffer = (*buffer).buffer;
        (!spa_buffer.is_null() && (*spa_buffer).n_datas >= 1).then(|| &mut *(*spa_buffer).datas)
    }
}

pub unsafe fn add(buffer: *mut pw_sys::pw_buffer, size: u32) -> rustix::io::Result<()> {
    let data = unsafe { first_data(buffer) }.ok_or(rustix::io::Errno::INVAL)?;
    let fd = memfd_create("wallpaper-rs", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
    ftruncate(&fd, u64::from(size))?;
    fcntl_add_seals(&fd, SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL)?;
    let read_write = ProtFlags::READ | ProtFlags::WRITE;
    let ptr = unsafe { mmap(std::ptr::null_mut(), size as usize, read_write, MapFlags::SHARED, &fd, 0)? };

    data.type_ = spa_sys::SPA_DATA_MemFd;
    data.flags = spa_sys::SPA_DATA_FLAG_READWRITE | spa_sys::SPA_DATA_FLAG_MAPPABLE;
    data.fd = i64::from(fd.into_raw_fd());
    data.mapoffset = 0;
    data.maxsize = size;
    data.data = ptr;
    Ok(())
}

pub unsafe fn remove(buffer: *mut pw_sys::pw_buffer) {
    let kinds = [spa_sys::SPA_DATA_MemFd, spa_sys::SPA_DATA_DmaBuf];
    let owned = |data: &&mut spa_sys::spa_data| kinds.contains(&data.type_) && data.fd >= 0;
    let Some(data) = unsafe { first_data(buffer) }.filter(owned) else { return };
    if !data.data.is_null() {
        let _ = unsafe { munmap(data.data, data.maxsize as usize) };
    }
    drop(unsafe { OwnedFd::from_raw_fd(data.fd as i32) });
    data.data = std::ptr::null_mut();
    data.fd = -1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::BorrowedFd;

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

        unsafe { std::slice::from_raw_parts_mut(data.data.cast::<u8>(), 4) }.copy_from_slice(b"wprs");
        let mut head = [0u8; 4];
        rustix::io::pread(unsafe { BorrowedFd::borrow_raw(data.fd as i32) }, &mut head, 0).unwrap();
        assert_eq!(&head, b"wprs");

        unsafe { remove(&mut buffer) };
        assert_eq!(data.fd, -1);
        assert!(data.data.is_null());
    }
}
