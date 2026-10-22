use image::codecs::jpeg::JpegEncoder;
use image::{GrayImage, ImageEncoder, Luma, Rgba, RgbaImage};
use std::path::PathBuf;
use wprs_render::decode_image;

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!("wprs-render-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn exif_orientation(value: u8) -> Vec<u8> {
    let mut exif = b"MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0".to_vec();
    exif.extend([value, 0, 0, 0, 0, 0, 0]);
    exif
}

#[test]
fn applies_exif_orientation() {
    let left_white = GrayImage::from_fn(16, 8, |x, _| Luma([if x < 8 { 255 } else { 0 }]));
    let mut jpeg = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, 100);
    encoder.set_exif_metadata(exif_orientation(6)).unwrap();
    encoder.write_image(left_white.as_raw(), 16, 8, image::ExtendedColorType::L8).unwrap();
    let file = TempFile::new("rotated.jpg", &jpeg);
    let image = decode_image(&file.0, 4096).unwrap();
    assert_eq!(image.dimensions(), (8, 16));
    assert!(image.get_pixel(4, 2).0[0] > 200, "{:?}", image.get_pixel(4, 2));
    assert!(image.get_pixel(4, 13).0[0] < 50, "{:?}", image.get_pixel(4, 13));
}

#[test]
fn downscales_oversized_images_once() {
    let wide =
        RgbaImage::from_fn(100, 50, |x, _| Rgba(if x < 50 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let mut png = Vec::new();
    wide.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let file = TempFile::new("wide.png", &png);
    assert_eq!(decode_image(&file.0, 4096).unwrap().dimensions(), (100, 50));
    let small = decode_image(&file.0, 40).unwrap();
    assert_eq!(small.dimensions(), (40, 20));
    assert_eq!(small.get_pixel(5, 10).0, [255, 0, 0, 255]);
    assert_eq!(decode_image(&file.0, 70).unwrap().dimensions(), (70, 35));
}

#[test]
fn reports_undecodable_files() {
    let file = TempFile::new("garbage.png", b"not an image at all");
    assert!(decode_image(&file.0, 4096).is_err());
}
