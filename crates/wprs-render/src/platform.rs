use image::RgbaImage;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

const BRANDS: [&[u8; 4]; 10] =
    [b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1", b"avif", b"avis"];

#[derive(Clone, Copy)]
pub struct Platform {
    pub decode: fn(&Path, u32) -> Option<RgbaImage>,
    pub size: fn(&Path) -> Option<(u32, u32)>,
}

static PLATFORM: OnceLock<Platform> = OnceLock::new();

pub fn set_platform_decoder(platform: Platform) -> bool {
    PLATFORM.set(platform).is_ok()
}

pub fn heif(path: &Path) -> bool {
    let mut header = [0; 12];
    let read = std::fs::File::open(path).and_then(|mut file| file.read_exact(&mut header));
    read.is_ok() && &header[4..8] == b"ftyp" && BRANDS.iter().any(|brand| header[8..12] == brand[..])
}

pub(crate) fn decode(path: &Path, max_edge: u32) -> Option<RgbaImage> {
    PLATFORM.get().filter(|_| heif(path)).and_then(|p| (p.decode)(path, max_edge))
}

pub(crate) fn size(path: &Path) -> Option<(u32, u32)> {
    PLATFORM.get().filter(|_| heif(path)).and_then(|p| (p.size)(path))
}
