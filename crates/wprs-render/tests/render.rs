//! GPU tests. They need a Vulkan or GL adapter (a software one is enough).
//! Set `WPRS_REQUIRE_GPU=1` to turn a missing adapter into a failure.

use image::{Rgba, RgbaImage};
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, Scene};

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

/// Converts a BGRA pixel to RGB for readable assertions.
fn rgb(p: [u8; 4]) -> [u8; 3] {
    [p[2], p[1], p[0]]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn background_only() {
    let Some(r) = renderer() else { return };
    let frame = r
        .render(
            Scene {
                image: None,
                fit: Fit::Fill,
                background: [10, 120, 250],
            },
            33,
            17,
        )
        .unwrap();
    assert_eq!((frame.width, frame.height, frame.stride), (33, 17, 132));
    assert_eq!(frame.data.len(), 33 * 17 * 4);
    assert!(close(rgb(frame.pixel(0, 0)), [10, 120, 250]));
    assert!(close(rgb(frame.pixel(32, 16)), [10, 120, 250]));
    assert_eq!(frame.pixel(5, 5)[3], 255);
}

#[test]
fn fit_letterboxes_with_background() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
    let frame = r
        .render(
            Scene {
                image: Some(&img),
                fit: Fit::Fit,
                background: [0, 0, 255],
            },
            40,
            20,
        )
        .unwrap();
    // Image occupies the middle 20x20; bars are 10 px wide on each side.
    assert!(close(rgb(frame.pixel(20, 10)), [255, 0, 0]));
    assert!(close(rgb(frame.pixel(2, 10)), [0, 0, 255]));
    assert!(close(rgb(frame.pixel(37, 10)), [0, 0, 255]));
}

#[test]
fn fill_covers_the_target() {
    let Some(r) = renderer() else { return };
    // Left half green, right half white: a 2:1 target shows both halves.
    let img = RgbaImage::from_fn(20, 10, |x, _| {
        if x < 10 {
            Rgba([0, 255, 0, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    });
    let frame = r
        .render(
            Scene {
                image: Some(&img),
                fit: Fit::Fill,
                background: [0, 0, 0],
            },
            64,
            32,
        )
        .unwrap();
    assert!(close(rgb(frame.pixel(4, 16)), [0, 255, 0]));
    assert!(close(rgb(frame.pixel(60, 16)), [255, 255, 255]));
    assert!(close(rgb(frame.pixel(4, 0)), [0, 255, 0]));
}

#[test]
fn rejects_invalid_sizes() {
    let Some(r) = renderer() else { return };
    let scene = Scene {
        image: None,
        fit: Fit::Fill,
        background: [0, 0, 0],
    };
    assert!(r.render(scene, 0, 10).is_err());
    assert!(r.render(scene, r.max_edge() + 1, 10).is_err());
}

#[test]
fn crossfade_blends_layers() {
    let Some(r) = renderer() else { return };
    let red = r.upload(
        &RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])),
        Fit::Fill,
        (8, 8),
    );
    let blue = r.upload(
        &RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])),
        Fit::Fill,
        (8, 8),
    );
    let target = r.create_target(8, 8).unwrap();
    let layer = |image, opacity| Layer {
        image: Some(image),
        fit: Fit::Fill,
        background: [0, 0, 0],
        opacity,
    };
    let start = r
        .render_layers(&target, &[layer(&red, 1.0), layer(&blue, 0.0)])
        .unwrap();
    assert!(close(rgb(start.pixel(4, 4)), [255, 0, 0]));
    let end = r
        .render_layers(&target, &[layer(&red, 1.0), layer(&blue, 1.0)])
        .unwrap();
    assert!(close(rgb(end.pixel(4, 4)), [0, 0, 255]));
    // Halfway the blend happens in linear light: both channels are
    // sqrt-ish brighter than 128 once encoded back to sRGB.
    let mid = rgb(r
        .render_layers(&target, &[layer(&red, 1.0), layer(&blue, 0.5)])
        .unwrap()
        .pixel(4, 4));
    assert!(mid[0].abs_diff(188) <= 3 && mid[2].abs_diff(188) <= 3, "{mid:?}");
    assert!(mid[1] <= 2);
}

#[test]
fn video_frames_update_in_place() {
    let Some(r) = renderer() else { return };
    let green = [0u8, 255, 0, 255].repeat(6 * 4);
    let mut tex = r.upload_rgba(&green, 6, 4, 24).unwrap();
    let target = r.create_target(6, 4).unwrap();
    let draw = |tex: &wprs_render::GpuImage| {
        let frame = r
            .render_layers(
                &target,
                &[Layer {
                    image: Some(tex),
                    fit: Fit::Stretch,
                    background: [0, 0, 0],
                    opacity: 1.0,
                }],
            )
            .unwrap();
        rgb(frame.pixel(3, 2))
    };
    assert!(close(draw(&tex), [0, 255, 0]));

    // Same size with padded rows: the texture is reused.
    let mut white = vec![0u8; 32 * 4];
    for row in white.as_chunks_mut::<32>().0 {
        row[..24].fill(255);
    }
    r.update_rgba(&mut tex, &white, 6, 4, 32).unwrap();
    assert_eq!(tex.size(), (6, 4));
    assert!(close(draw(&tex), [255, 255, 255]));

    // A new size replaces the texture.
    let red = [255u8, 0, 0, 255].repeat(3 * 2);
    r.update_rgba(&mut tex, &red, 3, 2, 12).unwrap();
    assert_eq!(tex.size(), (3, 2));
    assert!(close(draw(&tex), [255, 0, 0]));

    // Short buffers are rejected.
    assert!(r.update_rgba(&mut tex, &red[..8], 3, 2, 12).is_err());
}
