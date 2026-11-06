use crate::{Error, MAX_FRAMES, MAX_SIDE, Target, levels};
use basisu_c_sys::BasisTextureFormat;
use basisu_c_sys::common::{
    BU_COMP_FLAGS_GEN_MIPS_CLAMP, BU_COMP_FLAGS_GEN_MIPS_WRAP, BU_COMP_FLAGS_KTX2_OUTPUT,
    BU_COMP_FLAGS_KTX2_UASTC_ZSTD, BU_COMP_FLAGS_SRGB, BU_COMP_FLAGS_THREADED,
};
use basisu_c_sys::extra::types::{Extent3d, TextureViewDimension};
use basisu_c_sys::extra::{
    BasisuEncoder, BasisuEncoderParams, SourceImage, SourceImageFormat, basisu_encoder_init,
};
use image::{Rgba32FImage, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Etc1s,
    Uastc,
}

impl Codec {
    pub fn parse(name: &str) -> Option<Codec> {
        match name {
            "etc1s" => Some(Codec::Etc1s),
            "uastc" => Some(Codec::Uastc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub codec: Codec,
    pub quality: u8,
    pub srgb: bool,
    pub mips: bool,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Flat,
    Frames,
    Cube,
}

fn compress(source: SourceImage, o: &Options, shape: Shape, hdr: bool) -> Result<Vec<u8>, Error> {
    let Extent3d { width: w, height: h, depth_or_array_layers: n } = source.size;
    let fine = match shape {
        Shape::Flat => n == 1,
        Shape::Frames => (1..=MAX_FRAMES).contains(&n),
        Shape::Cube => n == 6 && w == h,
    };
    if w == 0 || h == 0 || w.max(h) > MAX_SIDE || !fine {
        return Err(Error::TooLarge);
    }
    basisu_encoder_init();
    let mut encoder = BasisuEncoder::new();
    encoder.set_image(source).map_err(|_| Error::Broken)?;
    let format = match (hdr, o.codec) {
        (true, _) => BasisTextureFormat::UastcHdr4x4,
        (false, Codec::Etc1s) => BasisTextureFormat::Etc1s,
        (false, Codec::Uastc) => BasisTextureFormat::UastcLdr4x4,
    };
    let mut flags = BU_COMP_FLAGS_THREADED | BU_COMP_FLAGS_KTX2_OUTPUT | BU_COMP_FLAGS_KTX2_UASTC_ZSTD;
    flags |= if o.srgb && !hdr { BU_COMP_FLAGS_SRGB } else { 0 };
    if o.mips && shape != Shape::Frames {
        flags |= if o.repeat { BU_COMP_FLAGS_GEN_MIPS_WRAP } else { BU_COMP_FLAGS_GEN_MIPS_CLAMP };
    }
    let params = BasisuEncoderParams {
        basis_tex_format: format,
        quality_level: i32::from(o.quality.clamp(1, 100)),
        effort_level: 2,
        flags_and_quality: flags,
        low_level_uastc_rdo_or_dct_quality: 0.0,
    };
    let kind = match shape {
        Shape::Flat => TextureViewDimension::D2,
        Shape::Frames => TextureViewDimension::D2Array,
        Shape::Cube => TextureViewDimension::Cube,
    };
    let bytes = encoder.compress(params.with_tex_type(kind)).map_err(|_| Error::Broken)?;
    crate::info(&bytes)?;
    Ok(crate::kv::with(&bytes, crate::kv::QUALITY, &[o.quality.clamp(1, 100)]))
}

pub fn encode(image: &RgbaImage, o: &Options) -> Result<Vec<u8>, Error> {
    encode_frames(std::slice::from_ref(image), o, Shape::Flat)
}

pub fn encode_frames(frames: &[RgbaImage], o: &Options, shape: Shape) -> Result<Vec<u8>, Error> {
    let (w, h) = frames.first().map_or((0, 0), RgbaImage::dimensions);
    if frames.iter().any(|f| f.dimensions() != (w, h)) || frames.len() > MAX_FRAMES as usize {
        return Err(Error::TooLarge);
    }
    let data: Vec<u8> = frames.iter().flat_map(|f| f.as_raw().iter().copied()).collect();
    let size = Extent3d { width: w, height: h, depth_or_array_layers: frames.len() as u32 };
    compress(SourceImage { data: &data, format: SourceImageFormat::Rgba8, size }, o, shape, false)
}

pub fn encode_hdr((w, h): (u32, u32), rgba: &[f32], o: &Options) -> Result<Vec<u8>, Error> {
    lit((w, h, 1), rgba, o, Shape::Flat)
}

pub fn encode_light(layers: &[Rgba32FImage], o: &Options) -> Result<Vec<u8>, Error> {
    let (w, h) = layers.first().map_or((0, 0), Rgba32FImage::dimensions);
    let shape = if layers.len() == 6 { Shape::Cube } else { Shape::Flat };
    if layers.iter().any(|l| l.dimensions() != (w, h)) || !matches!(layers.len(), 1 | 6) {
        return Err(Error::TooLarge);
    }
    let data: Vec<f32> = layers.iter().flat_map(|l| l.as_raw().iter().copied()).collect();
    lit((w, h, layers.len() as u32), &data, o, shape)
}

fn lit((w, h, n): (u32, u32, u32), rgba: &[f32], o: &Options, shape: Shape) -> Result<Vec<u8>, Error> {
    if rgba.len() != w as usize * h as usize * 4 * n as usize || rgba.iter().any(|v| !v.is_finite()) {
        return Err(Error::Broken);
    }
    let data: Vec<u8> = rgba.iter().flat_map(|v| v.max(0.0).to_ne_bytes()).collect();
    let size = Extent3d { width: w, height: h, depth_or_array_layers: n };
    compress(SourceImage { data: &data, format: SourceImageFormat::Rgba32Float, size }, o, shape, true)
}

fn rgba(pixels: Vec<u32>, (w, h): (u32, u32)) -> Option<RgbaImage> {
    let bytes = pixels.into_iter().flat_map(|p| {
        let [b, g, r, a] = p.to_le_bytes();
        [r, g, b, a]
    });
    RgbaImage::from_raw(w, h, bytes.collect())
}

pub fn look(bytes: &[u8], target: Target) -> Result<RgbaImage, Error> {
    let made = levels(bytes, target, MAX_SIDE, false)?;
    let (w, h) = (made.width as usize, made.height as usize);
    if target == Target::Rgba {
        return RgbaImage::from_raw(made.width, made.height, made.data).ok_or(Error::Broken);
    }
    let mut out = vec![0u32; w * h];
    let decoded = match target {
        Target::Bc7 => texture2ddecoder::decode_bc7(&made.data, w, h, &mut out),
        Target::Astc => texture2ddecoder::decode_astc(&made.data, w, h, 4, 4, &mut out),
        _ => texture2ddecoder::decode_etc2_rgba8(&made.data, w, h, &mut out),
    };
    decoded.map_err(|_| Error::Broken)?;
    rgba(out, (made.width, made.height)).ok_or(Error::Broken)
}
