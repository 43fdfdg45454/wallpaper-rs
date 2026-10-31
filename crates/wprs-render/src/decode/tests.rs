use super::*;
use image::codecs::jpeg::JpegEncoder;
use std::path::PathBuf;

#[test]
fn large_jpegs_are_decoded_at_a_reduced_scale_with_the_same_result_size() {
    let dir = tempfile::tempdir().unwrap();
    let (jpeg, png) = (dir.path().join("wide.jpg"), dir.path().join("wide.png"));
    let image = RgbImage::from_fn(1600, 1200, |x, _| image::Rgb([(x % 256) as u8, 128, 0]));
    image.save(&jpeg).unwrap();
    image.save(&png).unwrap();
    let (reduced, full) = decode_reduced(&jpeg, 300).expect("a reduced decode");
    assert_eq!((reduced.width(), reduced.height(), full), (300, 225, (1600, 1200)));
    assert!(decode_reduced(&jpeg, 1000).is_none() && decode_reduced(&png, 300).is_none());
    assert_eq!(decode_image(&jpeg, 300).unwrap().dimensions(), decode_image(&png, 300).unwrap().dimensions());
}

fn huge_jpeg(dir: &Path, name: &str, progressive: bool, (width, height): (u16, u16)) -> PathBuf {
    let mut bytes = Vec::new();
    let image = RgbImage::from_pixel(16, 16, image::Rgb([9, 9, 9]));
    image.write_with_encoder(JpegEncoder::new(&mut bytes)).unwrap();
    let at = bytes.windows(2).position(|m| m == [0xff, 0xc0]).expect("a baseline frame header");
    bytes[at + 1] = if progressive { 0xc2 } else { 0xc0 };
    bytes[at + 5..at + 9].copy_from_slice(&[height.to_be_bytes(), width.to_be_bytes()].concat());
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn tiny_jpegs_that_claim_huge_sizes_are_refused_at_once() {
    let dir = tempfile::tempdir().unwrap();
    for (progressive, size) in
        [(false, (60000, 60000)), (true, (60000, 60000)), (true, (12000, 10000)), (false, (40000, 3000))]
    {
        let path = huge_jpeg(dir.path(), "huge.jpg", progressive, size);
        let start = std::time::Instant::now();
        assert!(decode_reduced(&path, 3840).is_none(), "{size:?} was decoded at a reduced scale");
        assert!(size.0 < 60000 || decode_image(&path, 3840).is_err(), "{size:?} was decoded");
        assert!(start.elapsed() < std::time::Duration::from_secs(2), "{size:?} took {:?}", start.elapsed());
    }
    let same = huge_jpeg(dir.path(), "same.jpg", false, (16, 16));
    assert_eq!(decode_image(&same, 3840).unwrap().dimensions(), (16, 16));
}

#[test]
fn large_baseline_photos_are_decoded_at_a_reduced_scale() {
    let dir = tempfile::tempdir().unwrap();
    let photo = huge_jpeg(dir.path(), "photo.jpg", false, (16320, 12240));
    let start = std::time::Instant::now();
    let (reduced, full) = decode_reduced(&photo, 2040).expect("a reduced decode");
    let bound = std::time::Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 2 });
    assert!(start.elapsed() < bound, "it took {:?}", start.elapsed());
    assert_eq!(((reduced.width(), reduced.height()), full), ((2040, 1530), (16320, 12240)));
    assert_eq!(decode_image(&photo, 3840).unwrap().width(), 3840);
}
