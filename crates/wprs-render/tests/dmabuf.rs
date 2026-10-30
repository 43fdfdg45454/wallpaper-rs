#![cfg(target_os = "linux")]

use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use wprs_core::{Ages, Fit, Region};
use wprs_render::{Layer, Renderer};

fn color(background: [u8; 3]) -> Layer<'static> {
    Layer { image: None, fit: Fit::Fill, background, opacity: 1.0, uv: None, relief: None }
}

#[test]
fn frames_reach_exported_memory_in_the_layout_the_consumer_reads() {
    let r = match Renderer::shared() {
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
    let mut ages = Ages::default();
    let mut draw = |image: &wprs_render::Exported, background, damage| {
        ages.push(damage);
        r.render_exported(image, &[color(background)], &ages).unwrap()
    };
    draw(&buffer.image, [0, 255, 0], None);
    let buffer = match r.software() {
        true => buffer,
        false => dmabufs.allocate(70, 30, &[0]).expect("every GPU exports linear frames, drawn or copied"),
    };
    draw(&buffer.image, [0, 255, 0], None);
    let len = buffer.size as usize;
    let map =
        unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ, MapFlags::SHARED, &buffer.fd, 0) }.unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(map.cast::<u8>(), len) };
    let at = |x: u32, y: u32| {
        let i = (buffer.offset + y * buffer.stride + x * 4) as usize;
        [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
    };
    assert_eq!([at(0, 0), at(69, 29)], [[0, 255, 0, 255]; 2]);
    draw(&buffer.image, [255, 0, 0], None);
    assert_eq!([at(35, 0), at(0, 29)], [[0, 0, 255, 255]; 2]);
    draw(&buffer.image, [0, 0, 255], Some(Region { x: 10, y: 5, width: 20, height: 10 }));
    assert_eq!([at(15, 8), at(29, 14)], [[255, 0, 0, 255]; 2], "the damage was not drawn");
    assert_eq!([at(0, 0), at(30, 8), at(15, 15)], [[0, 0, 255, 255]; 3], "pixels outside the damage changed");
    unsafe { munmap(map, len) }.unwrap();
}
