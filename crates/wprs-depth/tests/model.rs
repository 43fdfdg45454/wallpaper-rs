use image::{Rgb, RgbImage};
use std::time::Instant;
use wprs_depth::{Estimator, Places};

const W: u32 = 192;
const H: u32 = 128;
const CENTER: [f32; 3] = [0.0, -0.45, -2.6];
const RADIUS: f32 = 0.55;

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    v.map(|c| c / length)
}

fn ray(x: u32, y: u32) -> (Rgb<u8>, bool) {
    let d = unit([
        (x as f32 + 0.5 - W as f32 / 2.0) / H as f32,
        -(y as f32 + 0.5 - H as f32 / 2.0) / H as f32,
        -1.0,
    ]);
    let b = dot(d, CENTER);
    let disc = b * b - (dot(CENTER, CENTER) - RADIUS * RADIUS);
    let sphere = if disc > 0.0 { b - disc.sqrt() } else { f32::MAX };
    let floor = if d[1] < -1e-4 { -1.0 / d[1] } else { f32::MAX };
    let shade = |c: [f32; 3]| Rgb(c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8));
    if sphere < floor {
        let normal = unit([0, 1, 2].map(|i| d[i] * sphere - CENTER[i]));
        let light = dot(normal, unit([0.4, 0.8, 0.3])).max(0.0);
        return (shade([0.85, 0.25, 0.2].map(|c| c * (0.2 + 0.8 * light))), true);
    }
    if floor < f32::MAX {
        let p = d.map(|c| c * floor);
        let tile = ((p[0] * 2.0).floor() as i32 + (p[2] * 2.0).floor() as i32) & 1 == 1;
        let (color, fog) = (if tile { [0.9, 0.9, 0.85] } else { [0.25, 0.3, 0.3] }, (-floor * 0.08).exp());
        let haze = [0.6, 0.7, 0.8];
        return (shade([0, 1, 2].map(|i| color[i] * fog + haze[i] * (1.0 - fog))), false);
    }
    (shade([0.55, 0.7, 0.95].map(|c| c * (0.6 + 0.4 * d[1].max(0.0)))), false)
}

#[test]
fn the_model_puts_the_near_ball_in_front_of_the_room() {
    let Some(model) = std::env::var_os("WPRS_DEPTH_MODEL") else {
        return eprintln!(
            "skipping: set WPRS_DEPTH_MODEL to the model file and ORT_DYLIB_PATH to ONNX Runtime"
        );
    };
    let cache = tempfile::tempdir().unwrap();
    unsafe {
        std::env::remove_var(wprs_depth::TELEMETRY);
        std::env::set_var("XDG_CACHE_HOME", cache.path());
    }
    let image = RgbImage::from_fn(W, H, |x, y| ray(x, y).0);
    let libraries = Places::default().library(None);
    let mut estimator = Estimator::open(model.as_ref(), &libraries, wprs_depth::threads()).unwrap();
    let started = Instant::now();
    let map = estimator.estimate(&image, 518).unwrap();
    let seconds = started.elapsed().as_secs_f32();
    let (sx, sy) = (map.width() as f32 / W as f32, map.height() as f32 / H as f32);
    let (mut near, mut far) = (Vec::new(), Vec::new());
    for (x, y, value) in map.enumerate_pixels() {
        let inside = ray((x as f32 / sx) as u32, (y as f32 / sy) as u32).1;
        if inside { &mut near } else { &mut far }.push(f32::from(value[0]) / 65535.0);
    }
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let (near, far) = (mean(&near), mean(&far));
    eprintln!("{}x{} in {seconds:.2} s: near {near:.3}, background {far:.3}", map.width(), map.height());
    assert!(near > far + 0.25, "near {near:.3}, background {far:.3}");
    assert!(!cache.path().join("Microsoft").exists(), "ONNX Runtime kept telemetry files");
}
