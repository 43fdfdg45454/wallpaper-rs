use image::{DynamicImage, Rgb, Rgb32FImage, Rgba, Rgba32FImage, RgbaImage};
use wprs_ktx2::light::{bright, decoded, eight, encoded, shown, wide};

#[test]
fn light_becomes_srgb_codes_and_comes_back() {
    assert_eq!(
        [encoded(0.0), encoded(0.5), encoded(1.0), encoded(7.0), encoded(-1.0)],
        [0, 188, 255, 255, 0]
    );
    assert_eq!(encoded(f32::NAN), 0);
    assert!((decoded(188) - 0.5).abs() < 0.003);
    assert!((0..=255).all(|c| encoded(decoded(c)) == c));
}

#[test]
fn float_pictures_are_shown_through_the_srgb_curve_unless_they_hold_data() {
    let light = DynamicImage::ImageRgb32F(Rgb32FImage::from_pixel(2, 1, Rgb([0.5, 4.0, 0.0])));
    assert!(bright(&light));
    assert_eq!(eight(&light, false).get_pixel(0, 0).0, [188, 255, 0, 255]);
    assert_eq!(eight(&light, true).get_pixel(0, 0).0, [128, 255, 0, 255]);
    let alpha = DynamicImage::ImageRgba32F(Rgba32FImage::from_pixel(1, 1, Rgba([0.5, 0.5, 0.5, 0.5])));
    assert_eq!(eight(&alpha, false).get_pixel(0, 0).0, [188, 188, 188, 128]);
    let made = shown(light.clone(), false);
    assert!(!bright(&made) && made.as_rgba8().is_some_and(|p| p.get_pixel(1, 0).0 == [188, 255, 0, 255]));
    assert!(bright(&shown(light, true)), "data keeps its floats");
    let plain = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([188, 0, 255, 200])));
    assert_eq!(eight(&plain, false).get_pixel(0, 0).0, [188, 0, 255, 200]);
    assert_eq!(shown(plain.clone(), false), plain);
}

#[test]
fn eight_bit_pictures_become_linear_light_beside_float_ones() {
    let plain = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([188, 255, 0, 51])));
    let [r, g, b, a] = wide(&plain, false).get_pixel(0, 0).0;
    assert!((r - 0.5).abs() < 0.003 && g == 1.0 && b == 0.0 && (a - 0.2).abs() < 1e-6, "{r} {g} {b} {a}");
    let [r, ..] = wide(&plain, true).get_pixel(0, 0).0;
    assert!((r - 188.0 / 255.0).abs() < 1e-6, "data stays as it is: {r}");
    let light = DynamicImage::ImageRgba32F(Rgba32FImage::from_pixel(1, 1, Rgba([6.0, 0.5, 0.0, 1.0])));
    assert_eq!(wide(&light, false).get_pixel(0, 0).0, [6.0, 0.5, 0.0, 1.0]);
}
