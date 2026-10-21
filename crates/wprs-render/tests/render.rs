use image::{Rgba, RgbaImage};
use wprs_core::Fit;
use wprs_render::{GpuImage, Layer, Renderer, Scene};

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

fn rgb(bgra: [u8; 4]) -> [u8; 3] {
    let [b, g, r, _] = bgra;
    [r, g, b]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn background_only() {
    let Some(r) = renderer() else { return };
    let frame = r.render(Scene { image: None, fit: Fit::Fill, background: [10, 120, 250] }, 33, 17).unwrap();
    assert_eq!((frame.width, frame.height, frame.stride), (33, 17, 132));
    assert_eq!(frame.data.len(), 33 * 17 * 4);
    assert!(close(rgb(frame.pixel(0, 0)), [10, 120, 250]));
    assert!(close(rgb(frame.pixel(32, 16)), [10, 120, 250]));
    assert_eq!(frame.pixel(5, 5)[3], 255);
}

#[test]
fn readback_strips_row_padding() {
    let Some(r) = renderer() else { return };
    for width in [1, 63, 64, 65] {
        let scene = Scene { image: None, fit: Fit::Fill, background: [200, 100, 50] };
        let frame = r.render(scene, width, 3).unwrap();
        assert_eq!((frame.stride, frame.data.len()), (width * 4, width as usize * 3 * 4));
        assert!(frame.data.as_chunks::<4>().0.iter().all(|&p| close(rgb(p), [200, 100, 50])));
    }
}

#[test]
fn fit_letterboxes_with_background() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
    let frame =
        r.render(Scene { image: Some(&img), fit: Fit::Fit, background: [0, 0, 255] }, 40, 20).unwrap();
    assert!(close(rgb(frame.pixel(20, 10)), [255, 0, 0]));
    assert!(close(rgb(frame.pixel(2, 10)), [0, 0, 255]));
    assert!(close(rgb(frame.pixel(37, 10)), [0, 0, 255]));
}

#[test]
fn fill_covers_the_target() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_fn(20, 10, |x, _| Rgba(if x < 10 { [0, 255, 0, 255] } else { [255; 4] }));
    let frame = r.render(Scene { image: Some(&img), fit: Fit::Fill, background: [0, 0, 0] }, 64, 32).unwrap();
    assert!(close(rgb(frame.pixel(4, 16)), [0, 255, 0]));
    assert!(close(rgb(frame.pixel(60, 16)), [255, 255, 255]));
    assert!(close(rgb(frame.pixel(4, 0)), [0, 255, 0]));
}

#[test]
fn rejects_invalid_sizes() {
    let Some(r) = renderer() else { return };
    let scene = Scene { image: None, fit: Fit::Fill, background: [0, 0, 0] };
    assert!(r.render(scene, 0, 10).is_err());
    assert!(r.render(scene, r.max_edge() + 1, 10).is_err());
}

#[test]
fn upload_downscales_only_when_needed() {
    let Some(r) = renderer() else { return };
    assert_eq!(r.upload(&RgbaImage::new(100, 100), Fit::Fill, (200, 200)).size(), (100, 100));
    assert_eq!(r.upload(&RgbaImage::new(390, 390), Fit::Fill, (200, 200)).size(), (390, 390));
    assert_eq!(r.upload(&RgbaImage::new(4000, 2000), Fit::Fit, (400, 400)).size(), (400, 200));
    let beyond_limits = RgbaImage::new(r.max_edge() * 2, 4);
    assert_eq!(r.upload(&beyond_limits, Fit::Center, (1000, 1000)).size(), (r.max_edge(), 2));
}

#[test]
fn crossfade_blends_layers() {
    let Some(r) = renderer() else { return };
    let red = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])), Fit::Fill, (8, 8));
    let blue = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])), Fit::Fill, (8, 8));
    let target = r.create_target(8, 8).unwrap();
    let layer = |image, opacity| Layer { image: Some(image), fit: Fit::Fill, background: [0, 0, 0], opacity };
    let start = r.render_layers(&target, &[layer(&red, 1.0), layer(&blue, 0.0)]).unwrap();
    assert!(close(rgb(start.pixel(4, 4)), [255, 0, 0]));
    let end = r.render_layers(&target, &[layer(&red, 1.0), layer(&blue, 1.0)]).unwrap();
    assert!(close(rgb(end.pixel(4, 4)), [0, 0, 255]));
    let is_linear_light_half = |channel: u8| channel.abs_diff(188) <= 3;
    let mid = rgb(r.render_layers(&target, &[layer(&red, 1.0), layer(&blue, 0.5)]).unwrap().pixel(4, 4));
    assert!(is_linear_light_half(mid[0]) && is_linear_light_half(mid[2]), "{mid:?}");
    assert!(mid[1] <= 2);
}

#[test]
fn video_frames_update_in_place() {
    let Some(r) = renderer() else { return };
    let green = [0u8, 255, 0, 255].repeat(6 * 4);
    let mut tex = r.upload_rgba(&green, 6, 4, 24).unwrap();
    let target = r.create_target(6, 4).unwrap();
    let draw = |tex: &GpuImage| {
        let layer = Layer { image: Some(tex), fit: Fit::Stretch, background: [0, 0, 0], opacity: 1.0 };
        rgb(r.render_layers(&target, &[layer]).unwrap().pixel(3, 2))
    };
    assert!(close(draw(&tex), [0, 255, 0]));

    let mut white_with_padded_rows = vec![0u8; 32 * 4];
    white_with_padded_rows.as_chunks_mut::<32>().0.iter_mut().for_each(|row| row[..24].fill(255));
    r.update_rgba(&mut tex, &white_with_padded_rows, 6, 4, 32).unwrap();
    assert_eq!(tex.size(), (6, 4));
    assert!(close(draw(&tex), [255, 255, 255]));

    let red = [255u8, 0, 0, 255].repeat(3 * 2);
    r.update_rgba(&mut tex, &red, 3, 2, 12).unwrap();
    assert_eq!(tex.size(), (3, 2));
    assert!(close(draw(&tex), [255, 0, 0]));
    assert!(r.update_rgba(&mut tex, &red[..8], 3, 2, 12).is_err());
}
