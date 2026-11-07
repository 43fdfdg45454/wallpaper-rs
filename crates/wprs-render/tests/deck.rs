use image::{ImageBuffer, Luma, Rgba, RgbaImage};
use wprs_core::Fit;
use wprs_render::{DepthMap, Layer, Relief, Renderer};

const W: u32 = 64;
const H: u32 = 32;

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn ground(x: u32) -> [u8; 3] {
    [(x * 4) as u8, 0, 128]
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 2)
}

fn square(x: u32, y: u32) -> bool {
    (24..40).contains(&x) && (8..24).contains(&y)
}

fn patch(x: u32, y: u32) -> bool {
    (8..16).contains(&x) && (4..28).contains(&y)
}

fn back() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, _| Rgba([ground(x)[0], 0, 128, 255]))
}

fn middle() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| if patch(x, y) { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 0, 0]) })
}

fn front() -> RgbaImage {
    RgbaImage::from_fn(
        W,
        H,
        |x, y| if square(x, y) { Rgba([255, 255, 255, 255]) } else { Rgba([0, 0, 0, 0]) },
    )
}

fn flat() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| match (square(x, y), patch(x, y)) {
        (true, _) => Rgba([255; 4]),
        (_, true) => Rgba([255, 0, 0, 255]),
        _ => back().get_pixel(x, 0).to_owned(),
    })
}

fn row(
    r: &Renderer,
    relief: impl FnOnce(&wprs_render::GpuImage) -> (wprs_render::GpuImage, f32),
) -> Vec<[u8; 3]> {
    let image = r.upload_rgba(flat().as_raw(), W, H, W * 4).unwrap();
    let (depth, shift) = relief(&image);
    let relief =
        Relief { depth: &depth, shift: [shift / W as f32, 0.0], focus: 0.0, steps: 16, overscan: 1.0 };
    let layer = Layer {
        image: Some(&image),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: Some(relief),
        flip: [false; 2],
        grade: None,
    };
    let (mut target, mut pixels) = (r.create_target(W, H).unwrap(), Vec::new());
    r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
    let all: Vec<[u8; 3]> = pixels.as_chunks::<4>().0.iter().map(|p| [p[2], p[1], p[0]]).collect();
    all[(H / 2 * W) as usize..((H / 2 + 1) * W) as usize].to_vec()
}

fn stacked(r: &Renderer, shift: f32) -> Vec<[u8; 3]> {
    row(r, |_| (r.upload_layers(&[back(), middle(), front()], &[0.0, 0.5, 1.0]).unwrap(), shift))
}

fn warped(r: &Renderer, shift: f32) -> Vec<[u8; 3]> {
    let depth: DepthMap = ImageBuffer::from_fn(W, H, |x, y| {
        Luma([if square(x, y) {
            u16::MAX
        } else if patch(x, y) {
            32768
        } else {
            0
        }])
    });
    row(r, |_| (r.upload_depth(&depth, None).unwrap(), shift))
}

#[test]
fn what_a_near_object_uncovers_shows_the_filled_background() {
    let Some(r) = renderer() else { return };
    let moved = stacked(r, 8.0);
    for x in [25, 27, 29, 31] {
        assert!(near(moved[x], ground(x as u32)), "x {x}: {:?} wants {:?}", moved[x], ground(x as u32));
    }
    assert!(near(moved[36], [255; 3]) && near(moved[46], [255; 3]) && near(moved[50], ground(50)));
    let old = warped(r, 8.0);
    assert!((25..32).any(|x| !near(old[x], ground(x as u32))), "the single image warp stretches here");
}

#[test]
fn each_layer_moves_by_its_own_depth_and_rest_shows_the_picture() {
    let Some(r) = renderer() else { return };
    let moved = stacked(r, 8.0);
    assert!(near(moved[18], [255, 0, 0]) && near(moved[12], [255, 0, 0]) && near(moved[9], ground(9)));
    assert!(near(moved[14], [255, 0, 0]) && !near(moved[22], [255, 0, 0]));
    let still = stacked(r, 0.0);
    assert!(near(still[30], [255; 3]) && near(still[10], [255, 0, 0]) && near(still[5], ground(5)));
}
