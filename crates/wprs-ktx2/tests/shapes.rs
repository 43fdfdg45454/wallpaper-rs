use image::{Rgba, RgbaImage};
use wprs_ktx2::{
    Codec, Error, Options, Shape, Target, durations, encode, encode_frames, encode_hdr, gpu_bytes, info,
    levels, quality, rgba, timed,
};

fn options() -> Options {
    Options { codec: Codec::Uastc, quality: 70, srgb: true, mips: false, repeat: false }
}

fn solid(w: u32, h: u32, c: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([c[0], c[1], c[2], 255]))
}

fn near(a: &[u8], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 12)
}

#[test]
fn frames_of_an_array_become_a_grid_with_their_timing() {
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255]];
    let frames: Vec<RgbaImage> = colors.iter().map(|c| solid(16, 8, *c)).collect();
    let bytes = encode_frames(&frames, &options(), Shape::Frames).unwrap();
    let found = info(&bytes).unwrap();
    assert_eq!((found.frames, found.cube, found.hdr, found.picture(0)), (5, false, false, (48, 16)));
    assert_eq!(wprs_ktx2::grid(5), (2, 3));
    let sheet = rgba(&bytes, 4096).unwrap();
    assert_eq!(sheet.dimensions(), (48, 16));
    for (i, c) in colors.iter().enumerate() {
        let (x, y) = ((i as u32 % 3) * 16 + 8, (i as u32 / 3) * 8 + 4);
        assert!(near(&sheet.get_pixel(x, y).0, *c), "frame {i}: {:?}", sheet.get_pixel(x, y));
    }
    let packed = levels(&bytes, Target::Bc7, 4096, true).unwrap();
    assert_eq!((packed.width, packed.height, packed.count, packed.data.len()), (48, 16, 1, 12 * 4 * 16));
    assert_eq!(durations(&bytes).unwrap(), vec![1.0 / 24.0; 5]);
    let slow = timed(&bytes, 0.25);
    assert_eq!(durations(&slow).unwrap(), vec![0.25; 5]);
    assert_eq!(rgba(&slow, 4096).unwrap(), sheet, "the timing does not move the pictures");
    assert_eq!(gpu_bytes(&found, Target::Rgba, true), 48 * 16 * 4);
}

#[test]
fn a_cube_map_becomes_a_panorama() {
    let faces: Vec<RgbaImage> = (0..6u8).map(|f| solid(16, 16, [f * 40, 255 - f * 40, 100])).collect();
    let bytes = encode_frames(&faces, &options(), Shape::Cube).unwrap();
    let found = info(&bytes).unwrap();
    assert_eq!((found.cube, found.frames, found.picture(0)), (true, 1, (64, 32)));
    let pano = rgba(&bytes, 4096).unwrap();
    assert_eq!(pano.dimensions(), (64, 32));
    assert!(near(&pano.get_pixel(32, 16).0, [160, 95, 100]), "front is +Z: {:?}", pano.get_pixel(32, 16));
    assert!(near(&pano.get_pixel(32, 0).0, [80, 175, 100]), "up is +Y: {:?}", pano.get_pixel(32, 0));
    assert!(near(&pano.get_pixel(48, 16).0, [0, 255, 100]), "right is +X: {:?}", pano.get_pixel(48, 16));
    assert_eq!(levels(&bytes, Target::Bc7, 4096, false).unwrap_err(), Error::Unsupported);
    assert!(encode_frames(&faces[..5], &options(), Shape::Cube).is_err());
}

#[test]
fn hdr_pictures_keep_their_light_on_half_floats_and_bc6h() {
    let (w, h) = (8u32, 8u32);
    let pixels: Vec<f32> =
        (0..w * h).flat_map(|i| [if i < 32 { 4.0 } else { 0.25 }, 0.5, 0.1, 1.0]).collect();
    let bytes = encode_hdr((w, h), &pixels, &options()).unwrap();
    let found = info(&bytes).unwrap();
    assert_eq!((found.hdr, found.codec), (true, "uastc-hdr"));
    let half = levels(&bytes, Target::Half, 4096, false).unwrap();
    let red = half::f16::from_le_bytes([half.data[0], half.data[1]]).to_f32();
    assert!((red - 4.0).abs() < 0.5, "{red}");
    assert_eq!(levels(&bytes, Target::Bc6h, 4096, false).unwrap().data.len(), 4 * 16);
    assert_eq!(levels(&bytes, Target::Bc7, 4096, false).unwrap_err(), Error::Unsupported);
    assert_eq!(rgba(&bytes, 4096).unwrap().get_pixel(0, 0).0[0], 255, "past white is clamped for the CPU");
    assert_eq!(gpu_bytes(&found, Target::Rgba, false), u64::from(w * h) * 8);
    assert!(encode_hdr((w, h), &[f32::NAN; 256], &options()).is_err());
    assert!(encode_hdr((w, h), &pixels[..8], &options()).is_err());
}

#[test]
fn the_quality_is_kept_in_the_file_and_other_keys_survive() {
    let bytes = encode(&solid(8, 8, [10, 20, 30]), &options()).unwrap();
    assert_eq!(quality(&bytes), Some(70));
    let again = wprs_ktx2::kv::with(&bytes, b"wprsQuality", &[42]);
    assert_eq!((quality(&again), info(&again).unwrap().width), (Some(42), 8));
    assert_eq!(rgba(&again, 64).unwrap(), rgba(&bytes, 64).unwrap());
    let pairs = wprs_ktx2::kv::pairs(&again);
    assert!(pairs.iter().any(|(k, _)| k.starts_with(b"KTXwriter")), "{pairs:?}");
    assert_eq!(wprs_ktx2::kv::with(b"not a texture", b"k", b"v"), b"not a texture");
}
