use crate::ffi::{Ptr, Text, get, text};
use std::ffi::{CStr, c_int};
use std::ptr::null_mut;

pub const NUL: *const i8 = std::ptr::null();

pub unsafe fn connect(object: Ptr, signal: &str, handler: *const (), data: Ptr) {
    let name = text(signal);
    let handler = unsafe { std::mem::transmute::<*const (), crate::ffi::Callback>(handler) };
    unsafe { (get().g_signal_connect_data)(object, name.as_ptr(), Some(handler), data, null_mut(), 0) };
}

pub unsafe fn string(value: Text) -> String {
    if value.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned()
    }
}

type Job = Box<dyn FnOnce()>;

unsafe extern "C" fn once(data: Ptr) -> c_int {
    let job = unsafe { Box::from_raw(data.cast::<Job>()) };
    job();
    0
}

pub fn later(ms: u32, job: impl FnOnce() + 'static) {
    let data = Box::into_raw(Box::new(Box::new(job) as Job)).cast();
    let call = once as unsafe extern "C" fn(Ptr) -> c_int as Ptr;
    unsafe {
        match ms {
            0 => (get().g_idle_add)(call, data),
            ms => (get().g_timeout_add)(ms, call, data),
        }
    };
}

pub unsafe fn has_property(object: Ptr, name: &str) -> bool {
    let class = unsafe { *object.cast::<Ptr>() };
    let name = text(name);
    !unsafe { (get().g_object_class_find_property)(class, name.as_ptr()) }.is_null()
}

pub unsafe fn set_bool(object: Ptr, name: &str, value: bool) {
    if unsafe { has_property(object, name) } {
        let name = text(name);
        unsafe { (get().g_object_set)(object, name.as_ptr(), c_int::from(value), NUL) };
    }
}

pub fn class_size(kind: crate::ffi::GType) -> u32 {
    let mut query = [0u32; 6];
    unsafe { (get().g_type_query)(kind, &mut query) };
    query[4]
}

pub fn strings(list: &[String]) -> (Vec<std::ffi::CString>, Vec<Text>) {
    let owned: Vec<_> = list.iter().map(|s| text(s)).collect();
    let pointers = owned.iter().map(|s| s.as_ptr()).chain(std::iter::once(NUL)).collect();
    (owned, pointers)
}
