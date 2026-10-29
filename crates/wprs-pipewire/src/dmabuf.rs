use crate::memfd::first_data;
use crate::params::DATA_SYNCOBJ;
use crate::pool::Pool;
use crate::shared::{Slot, lock};
use crate::syncobj::{Drm, Fences, Syncobj, sync_meta};
use pipewire::spa::sys as spa_sys;
use pipewire::stream::Stream;
use pipewire::sys::pw_buffer;
use std::any::Any;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
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

pub unsafe fn add(buffer: *mut pw_buffer, dmabuf: Dmabuf, height: u32, pool: &mut Pool) -> Option<usize> {
    let data = unsafe { first_data(buffer) }?;
    (data.type_, data.flags) = (spa_sys::SPA_DATA_DmaBuf, spa_sys::SPA_DATA_FLAG_READABLE);
    data.fd = i64::from(dmabuf.fd.into_raw_fd());
    (data.mapoffset, data.maxsize, data.data) = (0, dmabuf.size, std::ptr::null_mut());
    let chunk = unsafe { &mut *data.chunk };
    (chunk.offset, chunk.stride, chunk.size) = (dmabuf.offset, dmabuf.stride as i32, dmabuf.stride * height);
    let index = pool.insert(dmabuf.image, buffer);
    unsafe { (*buffer).user_data = std::ptr::without_provenance_mut(index) };
    Some(index)
}

unsafe fn datas<'a>(buffer: *mut pw_buffer) -> &'a mut [spa_sys::spa_data] {
    let Some(spa) = (unsafe { (*buffer).buffer.as_mut() }).filter(|s| !s.datas.is_null()) else {
        return &mut [];
    };
    unsafe { std::slice::from_raw_parts_mut(spa.datas, spa.n_datas as usize) }
}

pub unsafe fn fence(
    buffer: *mut pw_buffer,
    drm: &Arc<Drm>,
    pool: &mut Pool,
    index: usize,
) -> std::io::Result<bool> {
    let datas = unsafe { datas(buffer) };
    if datas.len() < 3 || unsafe { sync_meta(buffer) }.is_none() {
        return Ok(false);
    }
    let (acquire, release) = (Syncobj::new(drm)?, Syncobj::new(drm)?);
    for (data, timeline) in datas[1..3].iter_mut().zip([&acquire, &release]) {
        (data.type_, data.flags, data.mapoffset, data.maxsize) =
            (DATA_SYNCOBJ, spa_sys::SPA_DATA_FLAG_READABLE, 0, 0);
        (data.fd, data.data) = (i64::from(timeline.export()?.into_raw_fd()), std::ptr::null_mut());
    }
    pool.fence(index, Fences::new(Box::new(acquire), Box::new(release)));
    Ok(true)
}

pub unsafe fn remove(buffer: *mut pw_buffer, pool: &mut Pool) {
    pool.remove(unsafe { (*buffer).user_data }.addr(), buffer);
    for data in unsafe { datas(buffer) }.iter_mut().skip(1).filter(|d| d.type_ == DATA_SYNCOBJ && d.fd >= 0) {
        drop(unsafe { OwnedFd::from_raw_fd(data.fd as i32) });
        data.fd = -1;
    }
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
