//! GPU tests. They need a Vulkan or GL adapter (a software one is enough).
//! Set `WPRS_REQUIRE_GPU=1` to turn a missing adapter into a failure.

use image::{Rgba, RgbaImage};
use wprs_core::Fit;
use wprs_render::{Renderer, Scene};

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
