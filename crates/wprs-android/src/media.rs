use std::fs::File;
use std::io::Read;
use std::path::Path;

const STILL_BRANDS: [[u8; 4]; 10] =
    [*b"avif", *b"avis", *b"heic", *b"heix", *b"heim", *b"heis", *b"hevc", *b"hevx", *b"mif1", *b"msf1"];

pub(crate) fn video(header: &[u8]) -> bool {
    match header {
        [0x1a, 0x45, 0xdf, 0xa3, ..] => true,
        [_, _, _, _, b'f', b't', b'y', b'p', a, b, c, d, ..] => !STILL_BRANDS.contains(&[*a, *b, *c, *d]),
        _ => false,
    }
}

fn header(path: &Path) -> Option<[u8; 12]> {
    let mut header = [0; 12];
    File::open(path).and_then(|mut file| file.read_exact(&mut header)).ok().map(|()| header)
}

pub(crate) fn is_video(path: &Path) -> bool {
    header(path).is_some_and(|header| video(&header))
}

pub(crate) fn extension(path: &Path) -> Option<&'static str> {
    match header(path)? {
        [0x1a, 0x45, 0xdf, 0xa3, ..] => Some("mkv"),
        [b'P', b'K', 3, 4, ..] => Some("wpak"),
        [_, _, _, _, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f' | b's', ..] => Some("avif"),
        header @ [_, _, _, _, b'f', b't', b'y', b'p', ..] if !video(&header) => Some("heic"),
        header if video(&header) => Some("mp4"),
        header => image::guess_format(&header).ok()?.extensions_str().first().copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

    #[test]
    fn webm_and_mp4_are_videos() {
        let webm = Path::new(FIXTURES).join("magenta-64x32.webm");
        assert!(is_video(&webm));
        assert_eq!(extension(&webm), Some("mkv"));
        for brand in [b"isom", b"mp42", b"qt  ", b"3gp4", b"M4V "] {
            assert!(video(&[b"\0\0\0\x18ftyp".as_slice(), brand].concat()), "{brand:?}");
        }
    }

    #[test]
    fn images_and_short_files_are_not() {
        let png = Path::new(FIXTURES).join("green-64x32.png");
        assert!(!is_video(&png));
        assert_eq!(extension(&png), Some("png"));
        assert!(!is_video(&Path::new(FIXTURES).join("missing.webm")));
        assert_eq!(extension(&Path::new(FIXTURES).join("missing.webm")), None);
        for brand in [b"heic", b"avif", b"mif1"] {
            assert!(!video(&[b"\0\0\0\x1cftyp".as_slice(), brand].concat()), "{brand:?}");
        }
        let still = |name| extension(&Path::new(FIXTURES).join(name));
        assert_eq!([still("blue-64x32.heic"), still("blue-64x32.avif")], [Some("heic"), Some("avif")]);
        assert!(!video(b"\0\0\0\x18ftyp"));
        let zip = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(zip.path(), b"PK\x03\x04\x14\0\0\0\0\0\0\0").unwrap();
        assert_eq!((extension(zip.path()), is_video(zip.path())), (Some("wpak"), false));
        assert!(!video(b"\x89PNG\r\n\x1a\n\0\0\0\x0d"));
    }
}
