#![cfg(target_os = "linux")]

use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::os::fd::AsFd;
use wprs_core::{Ages, Fit, Region};
use wprs_render::{Layer, Renderer, Shared};

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

#[test]
fn decoded_frames_are_read_in_place_from_shared_memory() {
    let r = match Renderer::shared() {
        Ok(r) => r,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let (importable, dmabufs) = (r.importable(), r.dmabufs(false));
    let Some(dmabufs) = dmabufs.filter(|_| importable.contains(&0)) else {
        return assert!(!r.software() || importable.is_empty(), "software rendering reads no shared memory");
    };
    let buffer = dmabufs.allocate(64, 64, &[0]).unwrap();
    let (len, stride) = (buffer.size as usize, buffer.stride);
    let flags = ProtFlags::READ | ProtFlags::WRITE;
    let map = unsafe { mmap(std::ptr::null_mut(), len, flags, MapFlags::SHARED, &buffer.fd, 0) }.unwrap();
    let bytes = unsafe { std::slice::from_raw_parts_mut(map.cast::<u8>(), len) };
    let (luma, chroma) = (buffer.offset as usize, (buffer.offset + stride * 32) as usize);
    for y in 0..32 {
        let row = &mut bytes[luma + y * stride as usize..][..64];
        row.iter_mut().enumerate().for_each(|(x, v)| *v = if x < 32 { 235 } else { 16 });
    }
    (0..16).for_each(|y| bytes[chroma + y * stride as usize..][..64].fill(128));
    unsafe { munmap(map, len) }.unwrap();
    let mut slot = None;
    let mut look = |turn, fit| {
        let planes = [
            (buffer.fd.as_fd(), buffer.offset, stride),
            (buffer.fd.as_fd(), buffer.offset + stride * 32, stride),
        ];
        let frame = Shared { size: (64, 32), modifier: 0, planes, bt709: true, full: false, turn, fit };
        r.read_image(r.convert_shared(&mut slot, &frame).unwrap()).unwrap()
    };
    let level = |image: &image::RgbaImage, x, y| image.get_pixel(x, y).0[1];
    let plain = look(0, (64, 32));
    assert_eq!(plain.dimensions(), (64, 32));
    assert!(level(&plain, 8, 16) > 245 && level(&plain, 56, 16) < 10, "{:?}", plain.get_pixel(8, 16));
    let turned = look(2, (64, 32));
    assert!(level(&turned, 8, 16) < 10 && level(&turned, 56, 16) > 245, "a turned video is not turned");
    let upright = look(1, (64, 64));
    assert_eq!(upright.dimensions(), (32, 64));
    assert!(level(&upright, 16, 8) > 245 && level(&upright, 16, 56) < 10, "a rotated video is not rotated");
    let mirrored = [5, 7].map(|turn| look(turn, (64, 64)));
    let tops = mirrored.each_ref().map(|image| (level(image, 16, 8) > 245, level(image, 16, 56) > 245));
    assert_eq!(tops, [(false, true), (true, false)], "mirrored and rotated videos turn like videoflip");
    assert_eq!(look(0, (16, 16)).dimensions(), (16, 8), "a large video is not fitted to its screen");
}
