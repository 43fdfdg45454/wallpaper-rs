use super::renderer;
use std::time::Instant;
use wprs_core::filter::{Filter, Look};
use wprs_core::{Fit, Grade};
use wprs_render::{DeepPlanes, Layer, Renderer};

const LAYERS: usize = 32;

fn layers_ms(r: &Renderer, size: (u32, u32), grade: Option<Grade>) -> f64 {
    let image = r.upload_rgba(&[90, 140, 200, 255].repeat(64 * 64), 64, 64, 256).unwrap();
    let mut target = r.create_target(size.0, size.1).unwrap();
    let layer = Layer { grade, opacity: 0.9, ..super::layer(Some(&image), Fit::Stretch, [0; 3], 1.0) };
    let layers = [layer; LAYERS];
    r.draw_layers(&mut target, &layers).unwrap();
    r.finish().unwrap();
    let start = Instant::now();
    for _ in 0..20 {
        r.draw_layers(&mut target, &layers).unwrap();
    }
    r.finish().unwrap();
    start.elapsed().as_secs_f64() * 1e3 / (20 * LAYERS) as f64
}

fn deep_ms(r: &Renderer, (w, h): (u32, u32)) -> (f64, f64) {
    let (luma, chroma) = (vec![0x80u8; (w * h * 2) as usize], vec![0x80u8; (w * h / 2) as usize]);
    let planes = [(luma.as_slice(), w * 2), (chroma.as_slice(), w), (chroma.as_slice(), w)];
    let (mut slot, rgba) = (None, vec![128u8; (w * h * 4) as usize]);
    let deep = DeepPlanes { size: (w, h), planes, hlg: false, full: false };
    r.convert_deep(&mut slot, &deep).unwrap();
    let mut plain = r.upload_rgba(&rgba, w, h, w * 4).unwrap();
    r.finish().unwrap();
    let time = |f: &mut dyn FnMut()| {
        let start = Instant::now();
        (0..30).for_each(|_| f());
        r.finish().unwrap();
        start.elapsed().as_secs_f64() * 1e3 / 30.0
    };
    let hdr = time(&mut || {
        r.convert_deep(&mut slot, &deep).unwrap();
    });
    let sdr = time(&mut || r.update_rgba(&mut plain, &rgba, w, h, w * 4).unwrap());
    (hdr, sdr)
}

#[test]
#[ignore]
fn filters_and_tone_mapping_cost_on_this_gpu() {
    let Some(r) = renderer() else { return };
    let warm = Filter { look: Look::Golden, exposure: 0.3, saturation: 0.2, hue: 10.0, ..Filter::default() };
    for size in [(1920, 1080), (3840, 2160)] {
        let (plain, graded) = (layers_ms(r, size, None), layers_ms(r, size, warm.grade()));
        eprintln!(
            "screen pass {size:?}: {plain:.4} ms, with a filter {graded:.4} ms, filter {:.4} ms",
            graded - plain
        );
        let (hdr, sdr) = deep_ms(r, size);
        eprintln!(
            "video frame {size:?}: 10-bit upload and tone mapping {hdr:.3} ms, 8-bit RGBA upload {sdr:.3} ms"
        );
    }
}
