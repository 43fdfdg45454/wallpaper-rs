use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering::Relaxed};

struct Counted;

static WATCHED: AtomicI32 = AtomicI32::new(0);
static COUNT: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static ASIDE: AtomicBool = AtomicBool::new(false);

fn count(bytes: usize) {
    let watched = WATCHED.load(Relaxed);
    if watched != 0 && !ASIDE.load(Relaxed) && watched == unsafe { libc::gettid() } {
        COUNT.fetch_add(1, Relaxed);
        BYTES.fetch_add(bytes as u64, Relaxed);
    }
}

unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counted = Counted;

pub(crate) fn watch(start: bool) -> (u64, u64) {
    let counted = (COUNT.swap(0, Relaxed), BYTES.swap(0, Relaxed));
    WATCHED.store(if start { unsafe { libc::gettid() } } else { 0 }, Relaxed);
    counted
}

pub(crate) fn gpu<T>(calls: impl FnOnce() -> T) -> T {
    let watched = WATCHED.load(Relaxed) != 0 && WATCHED.load(Relaxed) == unsafe { libc::gettid() };
    ASIDE.store(watched, Relaxed);
    let made = calls();
    ASIDE.store(false, Relaxed);
    made
}
