use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;
use wprs_format::media::{Tone, tone_of};
use wprs_render::Coding;

type Coded = (PathBuf, Option<SystemTime>, u64, Coding);

static CODED: Mutex<Vec<Coded>> = Mutex::new(Vec::new());
static DECODED: Mutex<Vec<(PathBuf, Coding)>> = Mutex::new(Vec::new());
const KEPT: usize = 8;
const SDR: i32 = 3;
const ST2084: i32 = 6;
const HLG: i32 = 7;

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

pub(crate) fn coding(path: &Path) -> Coding {
    let meta = std::fs::metadata(path).ok();
    let stamp = (meta.as_ref().and_then(|m| m.modified().ok()), meta.as_ref().map_or(0, |m| m.len()));
    let mut read = CODED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(found) = read.iter().find(|r| r.0 == path && (r.1, r.2) == stamp) {
        return found.3;
    }
    let coding = match tone_of(path) {
        Some(Tone::Pq) => Coding::Pq,
        Some(Tone::Hlg) => Coding::Hlg,
        None => Coding::Sdr,
    };
    read.retain(|r| r.0 != path);
    if read.len() >= KEPT {
        read.remove(0);
    }
    read.push((path.to_owned(), stamp.0, stamp.1, coding));
    coding
}

pub(crate) fn decoded(path: &Path, transfer: i32) {
    let coding = match transfer {
        SDR => Some(Coding::Sdr),
        ST2084 => Some(Coding::Pq),
        HLG => Some(Coding::Hlg),
        _ => None,
    };
    let mut decoded = DECODED.lock().unwrap_or_else(PoisonError::into_inner);
    decoded.retain(|(p, _)| p != path);
    if decoded.len() >= KEPT {
        decoded.remove(0);
    }
    decoded.extend(coding.map(|c| (path.to_owned(), c)));
}

pub(crate) fn shown_as(path: &Path, coding: Coding) -> Coding {
    let decoded = DECODED.lock().unwrap_or_else(PoisonError::into_inner);
    decoded.iter().find(|(p, _)| p == path).map_or(coding, |(_, c)| *c)
}

pub(crate) fn bright(path: &Path) -> bool {
    shown_as(path, coding(path)) != Coding::Sdr
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

    #[test]
    fn hdr_videos_are_read_once_for_their_transfer() {
        let at = |name: &str| coding(&Path::new(FIXTURES).join(name));
        assert_eq!(
            [at("pq-64x32.mkv"), at("hlg-64x32.mkv"), at("magenta-64x32.webm")],
            [Coding::Pq, Coding::Hlg, Coding::Sdr]
        );
        let copy = tempfile::NamedTempFile::new().unwrap();
        std::fs::copy(Path::new(FIXTURES).join("pq-64x32.mkv"), copy.path()).unwrap();
        assert_eq!(coding(copy.path()), Coding::Pq);
        std::fs::copy(Path::new(FIXTURES).join("magenta-64x32.webm"), copy.path()).unwrap();
        assert_eq!(coding(copy.path()), Coding::Sdr, "a changed file is read again");
        assert_eq!(at("missing.webm"), Coding::Sdr);
    }

    #[test]
    fn frames_a_decoder_already_mapped_are_shown_as_they_come() {
        let path = Path::new("/clips/decoded.mkv");
        assert_eq!(shown_as(path, Coding::Pq), Coding::Pq);
        decoded(path, SDR);
        assert_eq!(shown_as(path, Coding::Pq), Coding::Sdr, "the decoder mapped the frames already");
        decoded(path, HLG);
        assert_eq!(shown_as(path, Coding::Pq), Coding::Hlg);
        decoded(path, 99);
        assert_eq!(shown_as(path, Coding::Pq), Coding::Pq, "an unknown transfer keeps the file's own");
    }

    #[test]
    fn a_video_the_decoder_maps_to_standard_colors_does_not_ask_for_hdr() {
        let copy = tempfile::NamedTempFile::new().unwrap();
        std::fs::copy(Path::new(FIXTURES).join("hlg-64x32.mkv"), copy.path()).unwrap();
        assert!(bright(copy.path()));
        decoded(copy.path(), SDR);
        assert!(!bright(copy.path()), "every frame comes in standard colors");
        assert!(!bright(&Path::new(FIXTURES).join("magenta-64x32.webm")));
    }
}
