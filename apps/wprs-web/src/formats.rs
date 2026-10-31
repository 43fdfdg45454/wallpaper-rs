use crate::ffi::{Ptr, get};
use std::cell::Cell;

const DISPLAY_CLASS: u32 = 496;
const PREFERRED_FORMATS: usize = 176;
const FORMATS: [u32; 2] = [0x3432_5258, 0x3432_5241];

type Preferred = unsafe extern "C" fn(Ptr) -> Ptr;

thread_local! {
    static OFFERED: Cell<Ptr> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn offer(display: Ptr, modifiers: &[u64]) -> Result<(), String> {
    if modifiers.is_empty() {
        return Ok(());
    }
    let a = get();
    let kind = unsafe { (a.wpe_display_headless_get_type)() };
    if crate::glib::class_size(kind) != DISPLAY_CLASS {
        return Err("this version of WPE WebKit lays out its displays differently".into());
    }
    unsafe {
        let device = (a.wpe_display_get_drm_device)(display);
        let builder = (a.wpe_buffer_formats_builder_new)(device);
        (a.wpe_buffer_formats_builder_append_group)(builder, device, 0);
        for fourcc in FORMATS {
            modifiers.iter().for_each(|m| (a.wpe_buffer_formats_builder_append_format)(builder, fourcc, *m));
        }
        OFFERED.with(|o| o.set((a.wpe_buffer_formats_builder_end)(builder)));
        let class = (a.g_type_class_ref)(kind).cast::<u8>();
        *class.add(PREFERRED_FORMATS).cast::<Option<Preferred>>() = Some(preferred);
    }
    Ok(())
}

unsafe extern "C" fn preferred(_display: Ptr) -> Ptr {
    OFFERED.with(Cell::get)
}
