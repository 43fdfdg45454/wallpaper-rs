use crate::{Error, MAX_SIDE, Target, levels};
use basisu_c_sys::BasisTextureFormat;
use basisu_c_sys::common::{
    BU_COMP_FLAGS_GEN_MIPS_CLAMP, BU_COMP_FLAGS_GEN_MIPS_WRAP, BU_COMP_FLAGS_KTX2_OUTPUT,
    BU_COMP_FLAGS_KTX2_UASTC_ZSTD, BU_COMP_FLAGS_SRGB, BU_COMP_FLAGS_THREADED,
};
use basisu_c_sys::extra::types::Extent3d;
use basisu_c_sys::extra::{
    BasisuEncoder, BasisuEncoderParams, SourceImage, SourceImageFormat, basisu_encoder_init,
};
use image::RgbaImage;
use image::imageops::FilterType;

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

fn fourfold(image: &RgbaImage) -> RgbaImage {
    let (w, h) = image.dimensions();
    let (fw, fh) = (w.div_ceil(4) * 4, h.div_ceil(4) * 4);
    match (fw, fh) == (w, h) {
        true => image.clone(),
        false => image::imageops::resize(image, fw, fh, FilterType::CatmullRom),
    }
}

pub fn encode(image: &RgbaImage, o: &Options) -> Result<Vec<u8>, Error> {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 || w.max(h) > MAX_SIDE {
        return Err(Error::TooLarge);
    }
    let image = fourfold(image);
    let size = Extent3d { width: image.width(), height: image.height(), depth_or_array_layers: 1 };
    basisu_encoder_init();
    let mut encoder = BasisuEncoder::new();
    let source = SourceImage { data: image.as_raw(), format: SourceImageFormat::Rgba8, size };
    encoder.set_image(source).map_err(|_| Error::Broken)?;
    let format = match o.codec {
        Codec::Etc1s => BasisTextureFormat::Etc1s,
        Codec::Uastc => BasisTextureFormat::UastcLdr4x4,
    };
    let mut flags = BU_COMP_FLAGS_THREADED | BU_COMP_FLAGS_KTX2_OUTPUT | BU_COMP_FLAGS_KTX2_UASTC_ZSTD;
    flags |= if o.srgb { BU_COMP_FLAGS_SRGB } else { 0 };
    if o.mips {
        flags |= if o.repeat { BU_COMP_FLAGS_GEN_MIPS_WRAP } else { BU_COMP_FLAGS_GEN_MIPS_CLAMP };
    }
    let params = BasisuEncoderParams {
        basis_tex_format: format,
        quality_level: i32::from(o.quality.clamp(1, 100)),
        effort_level: 2,
        flags_and_quality: flags,
        low_level_uastc_rdo_or_dct_quality: 0.0,
    };
    let bytes = encoder.compress(params).map_err(|_| Error::Broken)?;
    crate::info(&bytes)?;
    Ok(bytes)
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
