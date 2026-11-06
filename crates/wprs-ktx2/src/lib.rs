use basisu::{DecodeFlags, SourceFormat, Transcoder};
use image::RgbaImage;

#[cfg(feature = "encode")]
mod cube;
#[cfg(feature = "encode")]
mod encode;
pub mod kv;
pub mod light;
pub mod moving;
mod shapes;
mod target;
mod weight;

#[cfg(feature = "encode")]
pub use cube::{faces, plain_faces};
#[cfg(feature = "encode")]
pub use encode::{Codec, Options, Shape, encode, encode_frames, encode_hdr, encode_light, look};
pub use kv::{durations, paced, quality, timed};
pub use shapes::grid;
pub use target::{HDR_TARGETS, TARGETS, Target};
pub use weight::{gpu_bytes, plain};

pub const MAGIC: [u8; 12] = [0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A];
pub const MAX_SIDE: u32 = 16_384;
pub const MAX_LEVELS: u32 = 15;
pub const MAX_BYTES: u64 = 256 << 20;
pub const MAX_FRAMES: u32 = 256;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("it is not a KTX2 texture")]
    NotKtx2,
    #[error("the KTX2 texture is not a Basis Universal picture, cube map or array that the engine reads")]
    Unsupported,
    #[error("the KTX2 texture is larger than {MAX_SIDE} pixels a side, {MAX_FRAMES} frames or {} MiB", MAX_BYTES >> 20)]
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
    pub hdr: bool,
    pub frames: u32,
    pub cube: bool,
}

impl Info {
    pub fn picture(&self, level: u32) -> (u32, u32) {
        let (w, h) = sized(self, level);
        match (self.cube, grid(self.frames)) {
            (true, _) => (w * 4, w * 2),
            (false, (rows, columns)) => (w * columns, h * rows),
        }
    }

    pub fn shaped(&self) -> bool {
        self.cube || self.frames > 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    pub target: Target,
    pub width: u32,
    pub height: u32,
    pub padded: (u32, u32),
    pub count: u32,
    pub data: Vec<u8>,
}

pub fn is_ktx2(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

fn source(t: &Transcoder<'_>) -> Result<(&'static str, bool), Error> {
    match t.source_format() {
        SourceFormat::Etc1s => Ok(("etc1s", false)),
        SourceFormat::UastcLdr => Ok(("uastc", false)),
        SourceFormat::XuastcLdr(_) => Ok(("xuastc", false)),
        SourceFormat::UastcHdr4x4 => Ok(("uastc-hdr", true)),
        SourceFormat::UastcHdr6x6 | SourceFormat::AstcHdr6x6 => Ok(("astc-hdr", true)),
        _ => Err(Error::Unsupported),
    }
}

fn open(bytes: &[u8]) -> Result<(Transcoder<'_>, Info), Error> {
    if !is_ktx2(bytes) {
        return Err(Error::NotKtx2);
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let t = Transcoder::new(bytes).map_err(|_| Error::Unsupported)?;
    let (codec, hdr) = source(&t)?;
    let (width, height) = t.base_dimensions();
    let (frames, cube) = (t.layer_count().max(1), t.face_count() == 6);
    if !matches!(t.face_count(), 1 | 6) || (cube && (frames > 1 || width != height)) {
        return Err(Error::Unsupported);
    }
    if width.max(height) > MAX_SIDE || frames > MAX_FRAMES {
        return Err(Error::TooLarge);
    }
    if t.level_count() > MAX_LEVELS {
        return Err(Error::Levels);
    }
    let levels = t.level_count();
    let info = Info { width, height, levels, codec, alpha: t.has_alpha(), hdr, frames, cube };
    let smallest = info.picture(levels.saturating_sub(1));
    if smallest.0.max(smallest.1) > MAX_SIDE {
        return Err(Error::TooLarge);
    }
    Ok((t, info))
}

pub fn info(bytes: &[u8]) -> Result<Info, Error> {
    open(bytes).map(|(_, info)| info)
}

fn first(info: &Info, max_edge: u32) -> u32 {
    let fits = |l: &u32| info.picture(*l).0.max(info.picture(*l).1) <= max_edge.max(1);
    (0..info.levels).find(fits).unwrap_or(0)
}

fn sized(info: &Info, level: u32) -> (u32, u32) {
    ((info.width >> level).max(1), (info.height >> level).max(1))
}

pub fn supports(bytes: &[u8], target: Target) -> bool {
    open(bytes).is_ok_and(|(t, info)| info.hdr == target.hdr() && t.supports(target.basis()))
}

fn blocks(side: u32) -> u32 {
    side.max(1).div_ceil(4)
}

fn whole(sized: (u32, u32), padded: (u32, u32), level: u32) -> bool {
    blocks(sized.0 >> level) == blocks(padded.0 >> level)
        && blocks(sized.1 >> level) == blocks(padded.1 >> level)
}

pub fn levels(bytes: &[u8], target: Target, max_edge: u32, mips: bool) -> Result<Levels, Error> {
    let (t, info) = open(bytes)?;
    let start = first(&info, max_edge);
    let shown = info.picture(start);
    let fits = shown.0.max(shown.1) <= max_edge.max(1) && info.hdr == target.hdr();
    if !fits || !t.supports(target.basis()) {
        return Err(Error::Unsupported);
    }
    if info.cube {
        return shapes::panorama(&t, &info, start, target);
    }
    if info.frames > 1 {
        return shapes::sheet(&t, &info, start, target);
    }
    let (width, height) = sized(&info, start);
    let padded = match target.compressed() {
        true => (blocks(width) * 4, blocks(height) * 4),
        false => (width, height),
    };
    let wanted = if mips { info.levels - start } else { 1 };
    let count = (0..wanted).take_while(|l| whole((width, height), padded, *l)).count() as u32;
    let mut data = Vec::new();
    for level in start..start + count {
        data.extend(
            t.transcode(level, target.basis(), DecodeFlags::HIGH_QUALITY).map_err(|_| Error::Broken)?,
        );
    }
    Ok(Levels { target, width, height, padded, count, data })
}

pub fn rgba(bytes: &[u8], max_edge: u32) -> Result<RgbaImage, Error> {
    let hdr = info(bytes)?.hdr;
    let made = levels(bytes, if hdr { Target::Half } else { Target::Rgba }, max_edge, false)?;
    let data = if hdr { shapes::clamped(&made.data) } else { made.data };
    RgbaImage::from_raw(made.width, made.height, data).ok_or(Error::Broken)
}
