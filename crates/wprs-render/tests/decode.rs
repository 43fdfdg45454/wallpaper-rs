use image::codecs::jpeg::JpegEncoder;
use image::{GrayImage, ImageEncoder, Luma, Rgba, RgbaImage};
use std::path::PathBuf;
use wprs_render::{Pictures, SceneAssets, decode_image};

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

#[test]
fn animations_with_a_huge_canvas_are_refused_before_any_frame() {
    let mut gif = Vec::new();
    let frame = image::Frame::new(RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255])));
    image::codecs::gif::GifEncoder::new(&mut gif).encode_frame(frame).unwrap();
    assert_eq!(wprs_render::animation(&gif).map(|(a, _)| a.dimensions()).ok(), Some((1, 1)));
    gif[6..10].copy_from_slice(&[0xff; 4]);
    assert!(wprs_render::animation(&gif).is_err());
    let canvas =
        [&b"VP8X"[..], &10u32.to_le_bytes(), &[2, 0, 0, 0], &[0xff, 0x3f, 0], &[0xff, 0x3f, 0]].concat();
    let anim = [&b"ANIM"[..], &6u32.to_le_bytes(), &[0; 6]].concat();
    let size = (4 + canvas.len() + anim.len()) as u32;
    let webp = [&b"RIFF"[..], &size.to_le_bytes(), b"WEBP", &canvas, &anim].concat();
    assert!(wprs_render::animation(&webp).is_err());
}

#[test]
fn ids_that_share_an_animated_picture_decode_it_once_and_all_play() {
    let mut gif = Vec::new();
    let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
    for (color, ms) in [([255, 0, 0, 255], 100), ([0, 0, 255, 255], 300)] {
        let image = RgbaImage::from_pixel(4, 2, Rgba(color));
        let delay = image::Delay::from_numer_denom_ms(ms, 1);
        encoder.encode_frame(image::Frame::from_parts(image, 0, 0, delay)).unwrap();
    }
    drop(encoder);
    let asset: wprs_scene::Asset = serde_json::from_str(r#"{ "path": "a.gif", "animated": true }"#).unwrap();
    let (mut assets, mut pictures, mut reads) = (SceneAssets::default(), Pictures::default(), 0);
    for id in ["one", "two"] {
        let read = || -> Result<_, image::ImageError> {
            reads += 1;
            Ok(gif.clone())
        };
        let total =
            pictures.add((&mut assets, id, &asset), read, |b| Ok(image::load_from_memory(&b)?.into_rgba8()));
        assert_eq!(total.unwrap(), 8 * 2 * 4);
    }
    assert_eq!(reads, 1);
    assert_eq!(
        (assets.sheets.get("one"), assets.sheets.get("two")),
        (Some(&vec![0.1, 0.3]), Some(&vec![0.1, 0.3]))
    );
    assert!(std::sync::Arc::ptr_eq(&assets.images["one"], &assets.images["two"]));
}
