use image::{Rgba, RgbaImage};
use wprs_core::Fit;
use wprs_render::{GpuImage, Layer, Renderer, TargetLimits};

fn renderer() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn layer(image: Option<&GpuImage>, fit: Fit, background: [u8; 3], opacity: f32) -> Layer<'_> {
    Layer { image, fit, background, opacity, uv: None, relief: None }
}

fn draw(r: &Renderer, image: Option<&RgbaImage>, fit: Fit, background: [u8; 3], size: (u32, u32)) -> Vec<u8> {
    let uploaded = image.map(|img| r.upload(img, fit, size).unwrap());
    let mut target = r.create_target(size.0, size.1).unwrap();
    let mut pixels = Vec::new();
    r.render_layers(&mut target, &[layer(uploaded.as_ref(), fit, background, 1.0)], &mut pixels).unwrap();
    pixels
}

fn rgb(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * width + x) * 4) as usize;
    [pixels[i + 2], pixels[i + 1], pixels[i]]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn background_only() {
    let Some(r) = renderer() else { return };
    let pixels = draw(&r, None, Fit::Fill, [10, 120, 250], (33, 17));
    assert_eq!(pixels.len(), 33 * 17 * 4);
    assert!(close(rgb(&pixels, 33, 0, 0), [10, 120, 250]));
    assert!(close(rgb(&pixels, 33, 32, 16), [10, 120, 250]));
    assert_eq!(pixels[(5 * 33 + 5) * 4 + 3], 255);
}

#[test]
fn readback_strips_row_padding_and_keeps_row_order() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_fn(2, 2, |_, y| Rgba(if y == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    for width in [1, 63, 64, 65, 100] {
        let pixels = draw(&r, Some(&img), Fit::Stretch, [0, 0, 0], (width, 4));
        assert_eq!(pixels.len(), width as usize * 4 * 4);
        assert!((0..width).all(|x| close(rgb(&pixels, width, x, 0), [255, 0, 0])), "width {width}");
        assert!((0..width).all(|x| close(rgb(&pixels, width, x, 3), [0, 0, 255])), "width {width}");
    }
}

#[test]
fn targets_and_destinations_are_reused() {
    let Some(r) = renderer() else { return };
    let red = r.upload_rgba(&[255, 0, 0, 255].repeat(4), 2, 2, 8).unwrap();
    let blue = r.upload_rgba(&[0, 0, 255, 255].repeat(4), 2, 2, 8).unwrap();
    let mut target = r.create_target(65, 9).unwrap();
    let mut pixels = Vec::new();
    let mut frame = |image, fit| {
        r.render_layers(&mut target, &[layer(Some(image), fit, [0, 255, 0], 1.0)], &mut pixels).unwrap();
        (pixels.as_ptr(), rgb(&pixels, 65, 32, 4), rgb(&pixels, 65, 1, 4))
    };
    let (first, color, _) = frame(&red, Fit::Fill);
    assert!(close(color, [255, 0, 0]));
    let (second, color, _) = frame(&blue, Fit::Fill);
    assert_eq!(first, second);
    assert!(close(color, [0, 0, 255]));
    let (_, center, edge) = frame(&blue, Fit::Fit);
    assert!(close(center, [0, 0, 255]) && close(edge, [0, 255, 0]));
    let (_, center, _) = frame(&red, Fit::Fill);
    assert!(close(center, [255, 0, 0]));
}

#[test]
fn rejects_invalid_and_oversized_targets() {
    let Some(r) = renderer() else { return };
    assert!(r.create_target(0, 10).is_err());
    assert!(r.create_target(r.max_edge() + 1, 10).is_err());
    assert!(r.check_target(u32::MAX, u32::MAX).is_err());
    assert!(r.check_target(1, 1).is_ok() && r.check_target(8200, 8200).is_err());
    assert_eq!(r.limits().max_edge, r.max_edge());
}

#[test]
fn borrowed_readback_hands_out_padded_rows() {
    let Some(r) = renderer() else { return };
    let mut target = r.create_target(65, 3).unwrap();
    let layers = [layer(None, Fit::Fill, [0, 255, 0], 1.0)];
    let read = |w, h, stride, data: &[u8]| ((w, h, stride, data.len()), [data[642], data[641], data[640]]);
    let (geometry, center) = r.render_layers_with(&mut target, &layers, read).unwrap();
    assert_eq!(geometry, (65, 3, 512, 512 * 3));
    assert!(close(center, [0, 255, 0]));
}

#[test]
fn target_limits_count_padded_rows() {
    let limits = TargetLimits { max_edge: 4096, max_buffer_size: 4096 * 4 * 100 };
    assert!(limits.check(4096, 100).is_ok());
    assert!(limits.check(4096, 101).is_err());
    assert!(limits.check(0, 1).is_err() && limits.check(1, 0).is_err() && limits.check(4097, 1).is_err());
    let tiny = TargetLimits { max_edge: 64, max_buffer_size: 512 };
    assert!(tiny.check(1, 2).is_ok() && tiny.check(64, 2).is_ok());
    assert!(tiny.check(1, 3).is_err() && tiny.check(65, 1).is_err());
}

#[test]
fn fit_letterboxes_with_background() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
    let pixels = draw(&r, Some(&img), Fit::Fit, [0, 0, 255], (40, 20));
    assert!(close(rgb(&pixels, 40, 20, 10), [255, 0, 0]));
    assert!(close(rgb(&pixels, 40, 2, 10), [0, 0, 255]));
    assert!(close(rgb(&pixels, 40, 37, 10), [0, 0, 255]));
}

#[test]
fn fill_covers_the_target() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_fn(20, 10, |x, _| Rgba(if x < 10 { [0, 255, 0, 255] } else { [255; 4] }));
    let pixels = draw(&r, Some(&img), Fit::Fill, [0, 0, 0], (64, 32));
    assert!(close(rgb(&pixels, 64, 4, 16), [0, 255, 0]));
    assert!(close(rgb(&pixels, 64, 60, 16), [255, 255, 255]));
    assert!(close(rgb(&pixels, 64, 4, 0), [0, 255, 0]));
}

#[test]
fn upload_downscales_only_when_needed() {
    let Some(r) = renderer() else { return };
    let size = |w, h, fit, target| r.upload(&RgbaImage::new(w, h), fit, target).unwrap().size();
    assert_eq!(size(100, 100, Fit::Fill, (200, 200)), (100, 100));
    assert_eq!(size(390, 390, Fit::Fill, (200, 200)), (390, 390));
    assert_eq!(size(4000, 2000, Fit::Fit, (400, 400)), (400, 200));
    assert_eq!(size(r.max_edge() * 2, 4, Fit::Center, (1000, 1000)), (r.max_edge(), 2));
}

#[test]
fn crossfade_blends_layers() {
    let Some(r) = renderer() else { return };
    let red = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])), Fit::Fill, (8, 8)).unwrap();
    let blue = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])), Fit::Fill, (8, 8)).unwrap();
    let mut target = r.create_target(8, 8).unwrap();
    let mut pixels = Vec::new();
    let mut mix = |opacity| {
        let layers =
            [layer(Some(&red), Fit::Fill, [0; 3], 1.0), layer(Some(&blue), Fit::Fill, [0; 3], opacity)];
        r.render_layers(&mut target, &layers, &mut pixels).unwrap();
        rgb(&pixels, 8, 4, 4)
    };
    assert!(close(mix(0.0), [255, 0, 0]));
    assert!(close(mix(1.0), [0, 0, 255]));
    let is_linear_light_half = |channel: u8| channel.abs_diff(188) <= 3;
    let mid = mix(0.5);
    assert!(is_linear_light_half(mid[0]) && is_linear_light_half(mid[2]), "{mid:?}");
    assert!(mid[1] <= 2);
}

#[test]
fn video_frames_update_in_place() {
    let Some(r) = renderer() else { return };
    let green = [0u8, 255, 0, 255].repeat(6 * 4);
    let mut tex = r.upload_rgba(&green, 6, 4, 24).unwrap();
    let mut target = r.create_target(6, 4).unwrap();
    let mut pixels = Vec::new();
    let mut show = |tex: &GpuImage| {
        r.render_layers(&mut target, &[layer(Some(tex), Fit::Stretch, [0; 3], 1.0)], &mut pixels).unwrap();
        rgb(&pixels, 6, 3, 2)
    };
    assert!(close(show(&tex), [0, 255, 0]));

    let mut white_with_padded_rows = vec![0u8; 32 * 4];
    white_with_padded_rows.as_chunks_mut::<32>().0.iter_mut().for_each(|row| row[..24].fill(255));
    r.update_rgba(&mut tex, &white_with_padded_rows, 6, 4, 32).unwrap();
    assert_eq!(tex.size(), (6, 4));
    assert!(close(show(&tex), [255, 255, 255]));

    let red = [255u8, 0, 0, 255].repeat(3 * 2);
    r.update_rgba(&mut tex, &red, 3, 2, 12).unwrap();
    assert_eq!(tex.size(), (3, 2));
    assert!(close(show(&tex), [255, 0, 0]));
    assert!(r.update_rgba(&mut tex, &red[..8], 3, 2, 12).is_err());
}
