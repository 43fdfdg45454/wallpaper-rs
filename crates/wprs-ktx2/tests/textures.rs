use image::{Rgba, RgbaImage};
use wprs_ktx2::{Codec, Error, MAGIC, Options, TARGETS, Target, encode, gpu_bytes, info, levels, look, rgba};

fn picture(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 128, 255]))
}

fn options(codec: Codec) -> Options {
    Options { codec, quality: 80, srgb: true, mips: true, repeat: false }
}

fn error(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let pairs = a.as_raw().iter().zip(b.as_raw());
    pairs.map(|(x, y)| (f64::from(*x) - f64::from(*y)).abs()).sum::<f64>() / a.as_raw().len() as f64
}

#[test]
fn a_converted_picture_keeps_its_size_levels_and_looks_on_every_target() {
    let source = picture(64, 32);
    for codec in [Codec::Uastc, Codec::Etc1s] {
        let bytes = encode(&source, &options(codec)).unwrap();
        assert!(bytes.starts_with(&MAGIC));
        let found = info(&bytes).unwrap();
        assert_eq!((found.width, found.height, found.levels), (64, 32, 7), "{codec:?}");
        for target in TARGETS {
            let seen = look(&bytes, target).unwrap();
            assert_eq!(seen.dimensions(), (64, 32));
            let off = error(&source, &seen);
            assert!(off < 6.0, "{codec:?} {target:?}: {off}");
        }
        assert!(error(&source, &rgba(&bytes, 64).unwrap()) < 6.0);
    }
}

#[test]
fn levels_start_at_the_first_one_that_fits_and_count_the_memory() {
    let bytes = encode(&picture(64, 64), &options(Codec::Uastc)).unwrap();
    let made = levels(&bytes, Target::Bc7, 16, true).unwrap();
    assert_eq!((made.width, made.height, made.count), (16, 16, 5));
    assert_eq!(made.data.len(), 16 * 16 + 4 * 16 + 3 * 16);
    let one = levels(&bytes, Target::Rgba, 64, false).unwrap();
    assert_eq!((one.count, one.data.len()), (1, 64 * 64 * 4));
    assert_eq!(rgba(&bytes, 20).unwrap().dimensions(), (16, 16));
    let found = info(&bytes).unwrap();
    assert_eq!(gpu_bytes(&found, Target::Rgba, false), 64 * 64 * 4);
    assert_eq!(gpu_bytes(&found, Target::Bc7, false), 64 * 64);
}

#[test]
fn sides_that_are_not_a_multiple_of_four_are_padded_and_kept_exact() {
    let source = picture(30, 18);
    let bytes = encode(&source, &options(Codec::Uastc)).unwrap();
    let found = info(&bytes).unwrap();
    assert_eq!((found.width, found.height), (30, 18));
    let made = levels(&bytes, Target::Bc7, 4096, true).unwrap();
    assert_eq!((made.width, made.height, made.padded), (30, 18, (32, 20)));
    assert_eq!(made.count, 2, "only levels whose blocks match the padded texture are kept");
    assert_eq!(made.data.len(), (8 * 5 + 4 * 3) * 16);
    let plain = levels(&bytes, Target::Rgba, 4096, false).unwrap();
    assert_eq!((plain.padded, plain.data.len()), ((30, 18), 30 * 18 * 4));
    for target in TARGETS {
        let seen = look(&bytes, target).unwrap();
        assert_eq!(seen.dimensions(), (30, 18));
        assert!(error(&source, &seen) < 6.0, "{target:?}");
    }
}

#[test]
fn a_texture_without_mipmaps_has_one_level() {
    let bytes = encode(&picture(32, 32), &Options { mips: false, ..options(Codec::Etc1s) }).unwrap();
    assert_eq!(info(&bytes).unwrap().levels, 1);
    assert_eq!(levels(&bytes, Target::Astc, 8, true).unwrap_err(), Error::Unsupported);
}

#[test]
fn bad_textures_are_refused() {
    assert_eq!(info(b"\x89PNG\r\n\x1a\n").unwrap_err(), Error::NotKtx2);
    assert_eq!(info(&MAGIC).unwrap_err(), Error::Unsupported);
    let mut header = MAGIC.to_vec();
    header.resize(200, 0);
    assert_eq!(info(&header).unwrap_err(), Error::Unsupported);
    let bytes = encode(&picture(32, 32), &options(Codec::Uastc)).unwrap();
    assert!(info(&bytes[..bytes.len() / 2]).is_err() || rgba(&bytes[..bytes.len() / 2], 32).is_err());
    let mut huge = bytes.clone();
    huge[20..24].copy_from_slice(&40_000u32.to_le_bytes());
    assert!(info(&huge).is_err());
    let mut deep = bytes;
    deep[40..44].copy_from_slice(&16u32.to_le_bytes());
    assert!(info(&deep).is_err());
    assert_eq!(encode(&RgbaImage::new(16_385, 1), &options(Codec::Uastc)).unwrap_err(), Error::TooLarge);
}
