use image::{ImageBuffer, Luma, Rgba, RgbaImage};
use serde_json::json;
use wprs_core::edit::Mask;
use wprs_core::{Edit, Fit, UvRect};
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

fn halves(near: impl Fn(u32) -> bool) -> DepthMap {
    ImageBuffer::from_fn(W, H, |x, _| Luma([if near(x) { u16::MAX } else { 0 }]))
}

#[test]
fn depth_masks_follow_the_depth_map() {
    let Some(r) = renderer() else { return };
    let gray = RgbaImage::from_pixel(W, H, Rgba([100, 100, 100, 255]));
    let depth = halves(|x| x >= W / 2);
    let edit = Edit::from_value(json!({"version": 1,
        "masks": {"near": {"type": "depth", "range": [0.5, 1.0], "soft": 0.01}},
        "ops": [{"id": "ev", "type": "exposure", "ev": 1, "mask": "near"}]}))
    .unwrap();
    let out = r.read_image(&r.develop(&gray, &edit, Some(&depth), (W, H)).unwrap()).unwrap();
    let pixel = |x| out.get_pixel(x, H / 2).0;
    assert!(pixel(8)[0].abs_diff(100) <= 2 && pixel(56)[0] > 130, "{:?} {:?}", pixel(8), pixel(56));
    let mut inverted = edit.clone();
    inverted.masks.insert("near".into(), Mask::Depth { range: [0.5, 1.0], soft: 0.01, invert: true });
    for edit in [edit, inverted] {
        let blind = r.read_image(&r.develop(&gray, &edit, None, (W, H)).unwrap()).unwrap();
        assert!(blind.pixels().all(|p| p.0[0].abs_diff(100) <= 2), "a mask without a map covers nothing");
    }
}

#[test]
fn spanned_screens_stay_continuous_under_overscan() {
    let Some(r) = renderer() else { return };
    let image = RgbaImage::from_fn(W, H, |x, _| Rgba([(x * 4) as u8, 0, 128, 255]));
    let (image, depth) = (
        r.upload_rgba(image.as_raw(), W, H, W * 4).unwrap(),
        r.upload_depth(&halves(|_| false), None).unwrap(),
    );
    let half = |x: f32| {
        let relief = Relief { depth: &depth, shift: [0.05, 0.0], focus: 0.5, steps: 8, overscan: 1.1 };
        let uv = Some(UvRect { offset: [x, 0.0], scale: [0.5, 1.0], repeat: false });
        let layer = Layer {
            image: Some(&image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv,
            relief: Some(relief),
        };
        let (mut target, mut pixels) = (r.create_target(W / 2, H).unwrap(), Vec::new());
        r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        pixels.as_chunks::<4>().0.iter().map(|p| p[2]).collect::<Vec<u8>>()
    };
    let (left, right) = (half(0.0), half(0.5));
    let row = (H / 2 * W / 2) as usize;
    let (edge, start) = (left[row + W as usize / 2 - 1], right[row]);
    assert!(start > edge && start - edge <= 6, "left ends at {edge}, right starts at {start}");
}
