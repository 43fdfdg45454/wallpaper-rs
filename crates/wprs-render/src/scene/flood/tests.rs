use super::flood;
use image::{Rgba, RgbaImage};

#[test]
fn processor_fields_measure_signed_distances_from_the_edge() {
    let image = RgbaImage::from_fn(32, 16, |x, _| Rgba([255, 255, 255, if x < 16 { 255 } else { 0 }]));
    let field = flood(&image, 16);
    let d = |x: usize| field[8 * 32 + x][1];
    assert!(d(10) < -4.0 && d(10) > -7.0, "inside is negative: {}", d(10));
    assert!((d(24) - 8.0).abs() < 1.0, "outside grows with the distance: {}", d(24));
    assert_eq!((field[0][0], field[31][0]), (1.0, 0.0));
}

#[test]
fn an_empty_picture_has_no_edge_to_measure() {
    let field = flood(&RgbaImage::new(8, 8), 8);
    assert!(field.iter().all(|[c, d]| *c == 0.0 && *d >= 30000.0));
}
