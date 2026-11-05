use crate::MAX_FRAMES;
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::error::{ImageFormatHint, LimitError, LimitErrorKind};
use image::{AnimationDecoder, Frames, ImageDecoder, ImageError, ImageFormat, Limits, RgbaImage};
use std::io::Cursor;

pub const MAX_ANIMATION: u64 = 64 << 20;

fn unsupported(format: ImageFormat) -> ImageError {
    ImageError::Unsupported(ImageFormatHint::Exact(format).into())
}

fn too_large() -> ImageError {
    ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError))
}

fn bounded<'a, D: AnimationDecoder<'a> + ImageDecoder>(
    mut decoder: D,
    mut limits: Limits,
) -> Result<Frames<'a>, ImageError> {
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) * 4 > MAX_ANIMATION {
        return Err(too_large());
    }
    limits.max_alloc = Some(MAX_ANIMATION * 3);
    decoder.set_limits(limits)?;
    Ok(decoder.into_frames())
}

pub fn frames(bytes: &[u8], limits: Limits) -> Result<Vec<(RgbaImage, f32)>, ImageError> {
    let frames = match image::guess_format(bytes)? {
        ImageFormat::Gif => bounded(GifDecoder::new(Cursor::new(bytes))?, limits)?,
        format => bounded(WebPDecoder::new(Cursor::new(bytes)).map_err(|_| unsupported(format))?, limits)?,
    };
    let (mut list, mut bytes) = (Vec::new(), 0u64);
    for frame in frames.take(MAX_FRAMES as usize + 1) {
        let frame = frame?;
        let (num, den) = frame.delay().numer_denom_ms();
        let seconds = num as f32 / den.max(1) as f32 / 1000.0;
        let image = frame.into_buffer();
        bytes += u64::from(image.width()) * u64::from(image.height()) * 4;
        if bytes > MAX_ANIMATION || list.len() == MAX_FRAMES as usize {
            return Err(too_large());
        }
        list.push((image, if seconds < 0.02 { 0.1 } else { seconds.min(60.0) }));
    }
    Ok(list)
}
