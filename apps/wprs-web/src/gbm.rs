use std::ffi::{c_int, c_void};
use std::os::fd::AsRawFd;

type Ptr = *mut c_void;
const XRGB8888: u32 = 0x3432_5258;
const RENDERING: u32 = 1 << 2;

pub fn allocates(node: &str) -> bool {
    let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(node) else { return false };
    let Ok(lib) = (unsafe { libloading::Library::new("libgbm.so.1") }) else { return false };
    unsafe {
        let (Ok(open), Ok(create), Ok(free), Ok(close)) = (
            lib.get::<unsafe extern "C" fn(c_int) -> Ptr>(b"gbm_create_device"),
            lib.get::<unsafe extern "C" fn(Ptr, u32, u32, u32, u32) -> Ptr>(b"gbm_bo_create"),
            lib.get::<unsafe extern "C" fn(Ptr)>(b"gbm_bo_destroy"),
            lib.get::<unsafe extern "C" fn(Ptr)>(b"gbm_device_destroy"),
        ) else {
            return false;
        };
        let device = open(file.as_raw_fd());
        if device.is_null() {
            return false;
        }
        let buffer = create(device, 64, 64, XRGB8888, RENDERING);
        if !buffer.is_null() {
            free(buffer);
        }
        close(device);
        !buffer.is_null()
    }
}
