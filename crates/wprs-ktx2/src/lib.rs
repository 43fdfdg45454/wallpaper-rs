use basisu::{DecodeFlags, SourceFormat, TargetFormat, Transcoder};
use image::RgbaImage;

#[cfg(feature = "encode")]
mod encode;
mod target;

#[cfg(feature = "encode")]
pub use encode::{Codec, Options, encode, look};
pub use target::{TARGETS, Target};

pub const MAGIC: [u8; 12] = [0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A];
pub const MAX_SIDE: u32 = 16_384;
pub const MAX_LEVELS: u32 = 15;
pub const MAX_BYTES: u64 = 256 << 20;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("it is not a KTX2 texture")]
    NotKtx2,
    #[error("the KTX2 texture is not one Basis Universal picture that the engine reads")]
    Unsupported,
    #[error("the KTX2 texture is larger than {MAX_SIDE} pixels a side or {} MiB", MAX_BYTES >> 20)]
    TooLarge,
    #[error("the KTX2 texture has more than {MAX_LEVELS} levels")]
    Levels,
    #[error("the KTX2 texture is damaged")]
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub codec: &'static str,
    pub alpha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    pub target: Target,
    pub width: u32,
    pub height: u32,
    pub count: u32,
    pub data: Vec<u8>,
}

pub fn is_ktx2(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

fn open(bytes: &[u8]) -> Result<(Transcoder<'_>, Info), Error> {
    if !is_ktx2(bytes) {
        return Err(Error::NotKtx2);
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let t = Transcoder::new(bytes).map_err(|_| Error::Unsupported)?;
    let codec = match t.source_format() {
        SourceFormat::Etc1s => "etc1s",
        SourceFormat::UastcLdr => "uastc",
        SourceFormat::XuastcLdr(_) => "xuastc",
        _ => return Err(Error::Unsupported),
    };
    if t.is_video() || t.layer_count() > 1 || t.face_count() != 1 {
        return Err(Error::Unsupported);
    }
    let (width, height) = t.base_dimensions();
    if width.max(height) > MAX_SIDE {
        return Err(Error::TooLarge);
    }
    if t.level_count() > MAX_LEVELS {
        return Err(Error::Levels);
    }
    let info = Info { width, height, levels: t.level_count(), codec, alpha: t.has_alpha() };
    Ok((t, info))
}

pub fn info(bytes: &[u8]) -> Result<Info, Error> {
    open(bytes).map(|(_, info)| info)
}

fn first(info: &Info, max_edge: u32) -> u32 {
    (0..info.levels).find(|l| (info.width >> l).max(info.height >> l).max(1) <= max_edge.max(1)).unwrap_or(0)
}

fn sized(info: &Info, level: u32) -> (u32, u32) {
    ((info.width >> level).max(1), (info.height >> level).max(1))
}

pub fn supports(bytes: &[u8], target: Target) -> bool {
    open(bytes).is_ok_and(|(t, _)| t.supports(target.basis()))
}

pub fn levels(bytes: &[u8], target: Target, max_edge: u32, mips: bool) -> Result<Levels, Error> {
    let (t, info) = open(bytes)?;
    let start = first(&info, max_edge);
    let (width, height) = sized(&info, start);
    if (width.max(height) > max_edge.max(1)) || !t.supports(target.basis()) {
        return Err(Error::Unsupported);
    }
    if target != Target::Rgba && (width % 4 != 0 || height % 4 != 0) {
        return Err(Error::Unsupported);
    }
    let count = if mips { info.levels - start } else { 1 };
    let mut data = Vec::new();
    for level in start..start + count {
        data.extend(
            t.transcode(level, target.basis(), DecodeFlags::HIGH_QUALITY).map_err(|_| Error::Broken)?,
        );
    }
    Ok(Levels { target, width, height, count, data })
}

pub fn rgba(bytes: &[u8], max_edge: u32) -> Result<RgbaImage, Error> {
    let (t, info) = open(bytes)?;
    let level = first(&info, max_edge);
    let (w, h) = sized(&info, level);
    let data =
        t.transcode(level, TargetFormat::Rgba32, DecodeFlags::HIGH_QUALITY).map_err(|_| Error::Broken)?;
    RgbaImage::from_raw(w, h, data).ok_or(Error::Broken)
}

pub fn gpu_bytes(info: &Info, target: Target, mips: bool) -> u64 {
    let count = if mips { info.levels } else { 1 };
    (0..count).map(|l| target.bytes(sized(info, l))).sum()
}
