use crate::Renderer;
use crate::scene::{Image, Pixels};
use crate::texture::cropped;
use image::error::{DecodingError, ImageFormatHint, LimitError, LimitErrorKind};
use image::{DynamicImage, ImageError, ImageReader, ImageResult, Limits};
use std::io::{BufRead, Read, Seek};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{AstcBlock, AstcChannel, Features, TextureFormat as F};
use wprs_ktx2::{MAX_BYTES, MAX_SIDE, TARGETS, Target};

pub(crate) const COMPRESSED: Features = Features::TEXTURE_COMPRESSION_BC
    .union(Features::TEXTURE_COMPRESSION_ETC2)
    .union(Features::TEXTURE_COMPRESSION_ASTC);

fn format(target: Target, linear: bool) -> F {
    match (target, linear) {
        (Target::Bc7, false) => F::Bc7RgbaUnormSrgb,
        (Target::Bc7, true) => F::Bc7RgbaUnorm,
        (Target::Astc, false) => F::Astc { block: AstcBlock::B4x4, channel: AstcChannel::UnormSrgb },
        (Target::Astc, true) => F::Astc { block: AstcBlock::B4x4, channel: AstcChannel::Unorm },
        (Target::Etc2, false) => F::Etc2Rgba8UnormSrgb,
        (Target::Etc2, true) => F::Etc2Rgba8Unorm,
        (Target::Rgba, false) => F::Rgba8UnormSrgb,
        (Target::Rgba, true) => F::Rgba8Unorm,
    }
}

pub(crate) fn drawable(format: F) -> F {
    match (format.is_compressed(), format.is_srgb()) {
        (false, _) => format,
        (true, true) => F::Rgba8UnormSrgb,
        (true, false) => F::Rgba8Unorm,
    }
}

pub(crate) fn failure(e: wprs_ktx2::Error) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormatHint::Name("KTX2".into()), e))
}

pub(crate) fn picture<R: BufRead + Seek>(
    reader: ImageReader<R>,
    limits: &Limits,
) -> ImageResult<Result<DynamicImage, ImageReader<R>>> {
    let format = reader.format();
    let mut inner = reader.into_inner();
    if !wprs_ktx2::is_ktx2(inner.fill_buf()?) {
        let mut reader = ImageReader::new(inner);
        format.into_iter().for_each(|f| reader.set_format(f));
        return Ok(Err(reader));
    }
    let mut bytes = Vec::new();
    inner.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    let info = wprs_ktx2::info(&bytes).map_err(failure)?;
    let wide = limits.max_image_width.is_some_and(|w| info.width > w);
    let tall = limits.max_image_height.is_some_and(|h| info.height > h);
    let bytes_needed = u64::from(info.width) * u64::from(info.height) * 4;
    if wide || tall || limits.max_alloc.is_some_and(|a| bytes_needed > a) {
        return Err(ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError)));
    }
    Ok(Ok(DynamicImage::ImageRgba8(wprs_ktx2::rgba(&bytes, MAX_SIDE).map_err(failure)?)))
}

pub(crate) fn size(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    wprs_ktx2::info(&bytes).ok().map(|i| (i.width, i.height))
}

impl Renderer {
    pub fn ktx2_targets(&self) -> Vec<Target> {
        let features = self.device.features();
        let offered = |t: &Target| match t {
            Target::Bc7 => features.contains(Features::TEXTURE_COMPRESSION_BC),
            Target::Astc => features.contains(Features::TEXTURE_COMPRESSION_ASTC),
            Target::Etc2 => features.contains(Features::TEXTURE_COMPRESSION_ETC2),
            Target::Rgba => true,
        };
        TARGETS.into_iter().filter(offered).collect()
    }

    pub fn ktx2_target(&self) -> Target {
        let forced = self.packing.load(Ordering::Relaxed) as usize;
        let offered = self.ktx2_targets();
        let chosen = TARGETS.get(forced.wrapping_sub(1)).filter(|t| offered.contains(t));
        chosen.copied().unwrap_or(offered[0])
    }

    pub fn force_ktx2(&self, target: Option<Target>) {
        let index = target.and_then(|t| TARGETS.iter().position(|o| *o == t)).map_or(0, |i| i + 1);
        self.packing.store(index as u8, Ordering::Relaxed);
    }

    pub(crate) fn upload_packed(
        &self,
        pixels: &Arc<Pixels>,
        bytes: &[u8],
        linear: bool,
        mips: Option<bool>,
    ) -> Option<Image> {
        let info = wprs_ktx2::info(bytes).ok()?;
        let target = self.ktx2_target();
        let mipped = mips != Some(false) && info.levels > 1;
        if !target.compressed() || (mips == Some(true) && info.levels == 1) {
            return None;
        }
        let made = wprs_ktx2::levels(bytes, target, self.max_edge(), mipped).ok()?;
        let descriptor = wgpu::TextureDescriptor {
            label: Some("wprs-render ktx2"),
            size: wgpu::Extent3d { width: made.padded.0, height: made.padded.1, depth_or_array_layers: 1 },
            mip_level_count: made.count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format(target, linear),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &descriptor,
            TextureDataOrder::LayerMajor,
            &made.data,
        );
        let crop = [made.width as f32 / made.padded.0 as f32, made.height as f32 / made.padded.1 as f32];
        let image = Arc::new((cropped(texture, crop), [info.width as f32, info.height as f32]));
        self.keep(pixels, [linear, mipped], &image);
        Some(image)
    }
}
