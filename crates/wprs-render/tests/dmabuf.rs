#![cfg(target_os = "linux")]

use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use wprs_core::Fit;
use wprs_render::{Layer, Renderer};

fn color(background: [u8; 3]) -> Layer<'static> {
    Layer { image: None, fit: Fit::Fill, background, opacity: 1.0, uv: None, relief: None }
}

#[test]
fn frames_are_drawn_straight_into_exported_memory() {
    let r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let Some(dmabufs) = r.dmabufs(r.software()) else {
        return assert!(!r.adapter().ends_with("(Vulkan)") || !r.software(), "lavapipe cannot export memory");
    };
    let buffer = dmabufs.allocate(70, 30, dmabufs.modifiers()).unwrap();
    assert!(dmabufs.modifiers().contains(&buffer.modifier));
    assert!(buffer.stride >= 70 * 4 && buffer.size >= buffer.offset + buffer.stride * 30);
    assert!(dmabufs.allocate(70, 30, &[u64::MAX - 1]).is_err());
    r.render_exported(&buffer.image, &[color([0, 255, 0])]).unwrap();
    if !r.software() {
        return;
    }
    let len = buffer.size as usize;
    let map =
        unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ, MapFlags::SHARED, &buffer.fd, 0) }.unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(map.cast::<u8>(), len) };
    let at = |x: u32, y: u32| {
        let i = (buffer.offset + y * buffer.stride + x * 4) as usize;
        [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
    };
    assert_eq!([at(0, 0), at(69, 29)], [[0, 255, 0, 255]; 2]);
    r.render_exported(&buffer.image, &[color([255, 0, 0])]).unwrap();
    assert_eq!([at(35, 0), at(0, 29)], [[0, 0, 255, 255]; 2]);
    unsafe { munmap(map, len) }.unwrap();
}
