use crate::ffi::{Ptr, get};
use crate::ring::Ring;
use crate::state::{View, by_wpe, engine, live, say, send};
use std::ffi::c_int;
use std::os::fd::{AsFd, BorrowedFd};
use std::rc::Rc;
use std::time::{Duration, Instant};
use wprs_web::FromWeb;

type Render = unsafe extern "C" fn(Ptr, Ptr, Ptr, u32, *mut Ptr) -> c_int;
type Changed = unsafe extern "C" fn(Ptr, *mut Ptr, u32);

const VIEW_CLASS: u32 = 464;
const BUFFERS_CHANGED: usize = 136;
const RENDER_BUFFER: usize = 144;

pub fn patch() -> Result<(), String> {
    let a = get();
    let kind = unsafe { (a.wpe_view_headless_get_type)() };
    if crate::glib::class_size(kind) != VIEW_CLASS {
        return Err("this version of WPE WebKit lays out its views differently".into());
    }
    unsafe {
        let class = (a.g_type_class_ref)(kind).cast::<u8>();
        *class.add(RENDER_BUFFER).cast::<Option<Render>>() = Some(render);
        *class.add(BUFFERS_CHANGED).cast::<Option<Changed>>() = Some(changed);
    }
    Ok(())
}

unsafe extern "C" fn changed(wpe: Ptr, _buffers: *mut Ptr, _count: u32) {
    if let Some(view) = by_wpe(wpe) {
        view.known.borrow_mut().clear();
        send(&FromWeb::Forget { view: view.id });
    }
}

unsafe extern "C" fn render(wpe: Ptr, buffer: Ptr, _damage: Ptr, _count: u32, _error: *mut Ptr) -> c_int {
    let Some(view) = by_wpe(wpe) else { return 0 };
    let a = get();
    unsafe { (a.g_object_ref)(buffer) };
    let dmabuf = unsafe { (a.g_type_check_instance_is_a)(buffer, (a.wpe_buffer_dma_buf_get_type)()) } != 0;
    let shared = dmabuf && !live(|l| l.copy) && unsafe { share(&view, buffer) };
    if !shared {
        unsafe { copy(&view, buffer, dmabuf) };
        unsafe { (a.wpe_view_buffer_released)(wpe, buffer) };
    }
    pace(view, buffer);
    1
}

unsafe fn share(view: &View, buffer: Ptr) -> bool {
    let (a, Some(engine)) = (get(), engine()) else { return false };
    let modifier = unsafe { (a.wpe_buffer_dma_buf_get_modifier)(buffer) };
    let planes = unsafe { (a.wpe_buffer_dma_buf_get_n_planes)(buffer) };
    if planes != 1 || !engine.start.modifiers.contains(&modifier) {
        return false;
    }
    let known = view.known.borrow().iter().find(|(b, _)| *b == buffer).map(|(_, id)| *id);
    let id = match known {
        Some(id) => id,
        None => {
            let id = view.known.borrow().iter().map(|(_, id)| id + 1).max().unwrap_or(1);
            let size = unsafe {
                ((a.wpe_buffer_get_width)(buffer) as u32, (a.wpe_buffer_get_height)(buffer) as u32)
            };
            let fourcc = unsafe { (a.wpe_buffer_dma_buf_get_format)(buffer) };
            let (offset, stride) = unsafe {
                ((a.wpe_buffer_dma_buf_get_offset)(buffer, 0), (a.wpe_buffer_dma_buf_get_stride)(buffer, 0))
            };
            let fd = unsafe { BorrowedFd::borrow_raw((a.wpe_buffer_dma_buf_get_fd)(buffer, 0)) };
            let message = FromWeb::Buffer { view: view.id, id, size, fourcc, modifier, offset, stride };
            say(&engine.link, &message, &[fd]);
            view.known.borrow_mut().push((buffer, id));
            id
        }
    };
    view.held.borrow_mut().insert(id, buffer);
    send(&FromWeb::Frame { view: view.id, buffer: id });
    true
}

unsafe fn copy(view: &View, buffer: Ptr, dmabuf: bool) {
    let (a, Some(engine)) = (get(), engine()) else { return };
    let mut error = std::ptr::null_mut();
    let (bytes, stride) = match dmabuf {
        true => unsafe { ((a.wpe_buffer_import_to_pixels)(buffer, &mut error), 0) },
        false => unsafe { ((a.wpe_buffer_shm_get_data)(buffer), (a.wpe_buffer_shm_get_stride)(buffer)) },
    };
    if bytes.is_null() {
        unsafe { (a.g_error_free)(error) };
        return;
    }
    let size = unsafe { ((a.wpe_buffer_get_width)(buffer) as u32, (a.wpe_buffer_get_height)(buffer) as u32) };
    let stride = if stride == 0 { size.0 * 4 } else { stride };
    let mut length = 0;
    let data = unsafe { (a.g_bytes_get_data)(bytes, &mut length) };
    let data = unsafe { std::slice::from_raw_parts(data, length) };
    let mut ring = view.ring.borrow_mut();
    if !ring.as_ref().is_some_and(|r| r.fits(size, stride)) {
        *ring = Ring::new(size, stride).ok();
    }
    let Some(ring) = ring.as_mut() else { return };
    let Some(slot) = ring.write(data) else { return };
    let fresh = !std::mem::replace(&mut ring.sent, true);
    let message = FromWeb::Pixels { view: view.id, slot, size, stride, fresh };
    let fds = if fresh { vec![ring.fd.as_fd()] } else { Vec::new() };
    say(&engine.link, &message, &fds);
}

fn pace(view: Rc<View>, buffer: Ptr) {
    let now = Instant::now();
    let period = Duration::from_secs(1) / view.fps.get().clamp(1, 240);
    let due = view.last.get().map_or(now, |last| (last + period).max(now));
    view.last.set(Some(due));
    let wait = due.saturating_duration_since(now).as_millis() as u32;
    crate::glib::later(wait, move || unsafe {
        let a = get();
        (a.wpe_view_buffer_rendered)(view.wpe, buffer);
        (a.g_object_unref)(buffer);
    });
}

pub fn release(view: &View, id: u32) {
    if let Some(buffer) = view.held.borrow_mut().remove(&id) {
        unsafe { (get().wpe_view_buffer_released)(view.wpe, buffer) };
    }
}

pub fn freed(view: &View, slot: u32) {
    view.ring.borrow_mut().iter_mut().for_each(|r| r.release(slot));
}
