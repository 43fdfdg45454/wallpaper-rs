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

fn scene(near: impl Fn(u32, u32) -> bool, white: impl Fn(u32, u32) -> bool) -> (RgbaImage, DepthMap) {
    let image = RgbaImage::from_fn(W, H, |x, y| match white(x, y) {
        true => Rgba([255, 255, 255, 255]),
        false => Rgba([(x * 4) as u8, 0, 128, 255]),
    });
    let depth = ImageBuffer::from_fn(W, H, |x, y| Luma([if near(x, y) { u16::MAX } else { 0 }]));
    (image, depth)
}

fn draw(
    r: &Renderer,
    (image, depth): &(RgbaImage, DepthMap),
    shift: f32,
    focus: f32,
    steps: u32,
) -> Vec<[u8; 3]> {
    let image = r.upload_rgba(image.as_raw(), W, H, W * 4).unwrap();
    let depth = r.upload_depth(depth, None).unwrap();
    let relief = Relief { depth: &depth, shift: [shift / W as f32, 0.0], focus, steps, overscan: 1.0 };
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
    pixels.as_chunks::<4>().0.iter().map(|p| [p[2], p[1], p[0]]).collect()
}

fn at(pixels: &[[u8; 3]], x: u32) -> [u8; 3] {
    pixels[(H / 2 * W + x) as usize]
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 2)
}

const WHITE: [u8; 3] = [255; 3];

fn ground(x: u32) -> [u8; 3] {
    [(x * 4) as u8, 0, 128]
}

#[test]
fn the_foreground_hides_the_background_it_moves_over() {
    let Some(r) = renderer() else { return };
    let inside = |x: u32, y: u32| (24..40).contains(&x) && (8..24).contains(&y);
    let square = scene(inside, inside);
    let still = draw(r, &square, 0.0, 0.0, 16);
    let moved = draw(r, &square, 8.0, 0.0, 16);
    assert!(near(at(&still, 43), ground(43)), "{:?}", at(&still, 43));
    assert!(near(at(&moved, 43), WHITE), "{:?}", at(&moved, 43));
    for x in [10, 50] {
        assert!(near(at(&moved, x), ground(x)), "x {x}: {:?}", at(&moved, x));
    }
}

#[test]
fn the_focus_depth_stays_still() {
    let Some(r) = renderer() else { return };
    let inside = |x: u32, y: u32| (24..40).contains(&x) && (8..24).contains(&y);
    let square = scene(inside, inside);
    let near_still = draw(r, &square, 8.0, 1.0, 16);
    assert!(near(at(&near_still, 32), WHITE) && near(at(&near_still, 25), WHITE));
    assert!(near(at(&near_still, 10), ground(18)), "{:?}", at(&near_still, 10));
    let far_still = draw(r, &square, 8.0, 0.0, 16);
    assert!(near(at(&far_still, 10), ground(10)) && near(at(&far_still, 60), ground(60)));
}

#[test]
fn one_step_shifts_while_sixteen_steps_find_thin_walls() {
    let Some(r) = renderer() else { return };
    let bar = scene(|x, _| (30..32).contains(&x), |x, _| (28..34).contains(&x));
    let marched = draw(r, &bar, 16.0, 0.0, 16);
    let shifted = draw(r, &bar, 16.0, 0.0, 1);
    assert!(near(at(&marched, 40), WHITE), "{:?}", at(&marched, 40));
    assert!(near(at(&shifted, 40), ground(40)), "{:?}", at(&shifted, 40));
    assert!(near(at(&marched, 47), WHITE) && near(at(&shifted, 47), WHITE));
}
