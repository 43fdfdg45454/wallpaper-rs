use crate::memfd::first_data;
use crate::pool::Pool;
use crate::shared::{Slot, lock};
use pipewire::spa::sys as spa_sys;
use pipewire::stream::Stream;
use pipewire::sys::pw_buffer;
use std::any::Any;
use std::os::fd::{IntoRawFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

pub type Image = Arc<dyn Any + Send + Sync>;

pub struct Dmabuf {
    pub fd: OwnedFd,
    pub modifier: u64,
    pub offset: u32,
    pub stride: u32,
    pub size: u32,
    pub image: Image,
}

pub trait Allocate: Send + Sync {
    fn modifiers(&self) -> &[u64];
    fn allocate(&self, width: u32, height: u32, modifiers: &[u64]) -> Option<Dmabuf>;
}

pub unsafe fn offered(buffer: *mut pw_buffer) -> bool {
    unsafe { first_data(buffer) }.is_some_and(|data| data.type_ & (1 << spa_sys::SPA_DATA_DmaBuf) != 0)
}

pub unsafe fn add(buffer: *mut pw_buffer, dmabuf: Dmabuf, height: u32, pool: &mut Pool) -> bool {
    let Some(data) = (unsafe { first_data(buffer) }) else { return false };
    (data.type_, data.flags) = (spa_sys::SPA_DATA_DmaBuf, spa_sys::SPA_DATA_FLAG_READABLE);
    data.fd = i64::from(dmabuf.fd.into_raw_fd());
    (data.mapoffset, data.maxsize, data.data) = (0, dmabuf.size, std::ptr::null_mut());
    let chunk = unsafe { &mut *data.chunk };
    (chunk.offset, chunk.stride, chunk.size) = (dmabuf.offset, dmabuf.stride as i32, dmabuf.stride * height);
    let index = pool.insert(dmabuf.image, buffer);
    unsafe { (*buffer).user_data = std::ptr::without_provenance_mut(index) };
    true
}

pub unsafe fn remove(buffer: *mut pw_buffer, pool: &mut Pool) {
    pool.remove(unsafe { (*buffer).user_data }.addr(), buffer);
}

pub fn process(stream: &Stream, slot: &Slot) -> Option<bool> {
    let mut pool = lock(&slot.pool);
    if !pool.active {
        return None;
    }
    let dequeue = || Some(unsafe { stream.dequeue_raw_buffer() }).filter(|b| !b.is_null());
    std::iter::from_fn(dequeue).for_each(|buffer| pool.returned(unsafe { (*buffer).user_data }.addr()));
    if let Some(buffer) = pool.next(slot.dirty.swap(false, SeqCst)) {
        unsafe { stream.queue_raw_buffer(buffer) };
    }
    Some(pool.starved())
}
