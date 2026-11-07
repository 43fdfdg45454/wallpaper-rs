use image::{GrayImage, Luma, Rgb, RgbImage};
use wprs_depth::{Device, Inpainter};

#[test]
fn lama_fills_a_hole_with_the_surroundings() {
    let (Some(model), Some(_)) = (std::env::var_os("WPRS_LAMA_MODEL"), std::env::var_os("ORT_DYLIB_PATH"))
    else {
        return eprintln!(
            "skipping: set WPRS_LAMA_MODEL to the LaMa file and ORT_DYLIB_PATH to ONNX Runtime"
        );
    };
    let libraries = wprs_depth::Places::default().library(None);
    let mut lama =
        Inpainter::from_file(model.as_ref(), &libraries, wprs_depth::threads(), Device::Cpu).unwrap();
    let image = RgbImage::from_fn(512, 512, |x, y| {
        if x < 256 { Rgb([200, 40 + (y / 4) as u8 / 2, 40]) } else { Rgb([30, 60, 180 - (y / 6) as u8]) }
    });
    let hole = |x: u32, y: u32| (200..312).contains(&x) && (200..312).contains(&y);
    let mask = GrayImage::from_fn(512, 512, |x, y| Luma([if hole(x, y) { 255 } else { 0 }]));
    let mut input = image.clone();
    input.enumerate_pixels_mut().filter(|(x, y, _)| hole(*x, *y)).for_each(|(_, _, p)| *p = Rgb([0; 3]));
    let started = std::time::Instant::now();
    let out = lama.run(&input, &mask).unwrap();
    eprintln!("lama ran in {:?}", started.elapsed());
    let mean = |img: &RgbImage, x0: u32, x1: u32| {
        let px: Vec<_> = (x0..x1)
            .flat_map(|x| (210..300).map(move |y| (x, y)))
            .map(|(x, y)| img.get_pixel(x, y).0)
            .collect();
        [0, 1, 2].map(|c| px.iter().map(|p| u32::from(p[c])).sum::<u32>() / px.len() as u32)
    };
    let (left, right) = (mean(&out, 205, 240), mean(&out, 275, 310));
    eprintln!("left {left:?} right {right:?} want {:?} {:?}", mean(&image, 205, 240), mean(&image, 275, 310));
    assert!(left[0] > 120 && left[2] < 120, "{left:?}");
    assert!(right[2] > 100 && right[0] < 120, "{right:?}");
    assert_eq!(out.get_pixel(10, 10), image.get_pixel(10, 10));
}
