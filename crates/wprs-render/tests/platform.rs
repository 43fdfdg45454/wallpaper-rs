use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use wprs_render::{Platform, decode_image, heif, image_size, set_platform_decoder};

static ASKED: AtomicU32 = AtomicU32::new(0);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(name)
}

fn decode(_: &Path, max_edge: u32) -> Option<RgbaImage> {
    ASKED.fetch_add(1, Relaxed);
    Some(RgbaImage::from_pixel(max_edge.min(64), max_edge.min(32), Rgba([40, 100, 200, 255])))
}

#[test]
fn heif_and_avif_photos_go_to_the_platform_decoder() {
    let (heic, avif, png) =
        (fixture("blue-64x32.heic"), fixture("blue-64x32.avif"), fixture("green-64x32.png"));
    assert!(heif(&heic) && heif(&avif) && !heif(&png) && !heif(&fixture("magenta-64x32.webm")));
    assert!(decode_image(&heic, 512).is_err(), "without a platform decoder they are refused");
    assert!(set_platform_decoder(Platform { decode, size: |_| Some((64, 32)) }));
    for photo in [&heic, &avif] {
        let decoded = decode_image(photo, 512).unwrap();
        assert_eq!((decoded.dimensions(), image_size(photo).unwrap()), ((64, 32), (64, 32)));
        assert_eq!(decoded.get_pixel(0, 0).0, [40, 100, 200, 255]);
    }
    assert_eq!(decode_image(&avif, 16).unwrap().width(), 16, "the platform decodes at the size asked");
    let asked = ASKED.load(Relaxed);
    assert_eq!(decode_image(&png, 512).unwrap().dimensions(), (64, 32));
    assert_eq!(ASKED.load(Relaxed), asked, "pictures the crate reads never reach the platform");
}
