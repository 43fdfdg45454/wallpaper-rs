use crate::SceneAssets;
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::error::{ImageFormatHint, LimitError, LimitErrorKind};
use image::{AnimationDecoder, Frames, ImageDecoder, ImageError, ImageFormat, RgbaImage};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use wprs_scene::{Asset, MAX_FRAMES};

pub const MAX_ANIMATION: u64 = 64 << 20;

fn unsupported(format: ImageFormat) -> ImageError {
    ImageError::Unsupported(ImageFormatHint::Exact(format).into())
}

fn too_large() -> ImageError {
    ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError))
}

fn bounded<'a, D: AnimationDecoder<'a> + ImageDecoder>(mut decoder: D) -> Result<Frames<'a>, ImageError> {
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) * 4 > MAX_ANIMATION {
        return Err(too_large());
    }
    let mut limits = crate::picture_limits();
    limits.max_alloc = Some(MAX_ANIMATION * 3);
    decoder.set_limits(limits)?;
    Ok(decoder.into_frames())
}

pub fn animation(bytes: &[u8]) -> Result<(RgbaImage, Vec<f32>), ImageError> {
    let frames = match image::guess_format(bytes)? {
        ImageFormat::Gif => bounded(GifDecoder::new(Cursor::new(bytes))?)?,
        format => bounded(WebPDecoder::new(Cursor::new(bytes)).map_err(|_| unsupported(format))?)?,
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
    let (w, h) = list.first().map_or((1, 1), |(i, _)| i.dimensions());
    let (rows, columns) = crate::scene::layout(list.len());
    let mut atlas = RgbaImage::new(w * columns, h * rows);
    for (i, (image, _)) in list.iter().enumerate() {
        let (x, y) = ((i as u32 % columns) * w, (i as u32 / columns) * h);
        image::imageops::replace(&mut atlas, image, i64::from(x), i64::from(y));
    }
    Ok((atlas, list.into_iter().map(|(_, d)| d).collect()))
}

#[derive(Default)]
pub struct Pictures(HashMap<String, (Arc<crate::Pixels>, Option<Vec<f32>>)>);

impl Pictures {
    pub fn add<E: From<ImageError>>(
        &mut self,
        (assets, id, asset): (&mut SceneAssets, &str, &Asset),
        read: impl FnOnce() -> Result<Vec<u8>, E>,
        still: impl FnOnce(Vec<u8>) -> Result<RgbaImage, E>,
    ) -> Result<u64, E> {
        let key = format!("{}#{}", asset.path, asset.animated);
        let (image, sheet) = match self.0.get(&key) {
            Some(found) => found.clone(),
            None => {
                let bytes = read()?;
                let (image, sheet) = match asset.animated {
                    true => animation(&bytes).map(|(atlas, durations)| (atlas, Some(durations)))?,
                    false => (still(bytes)?, None),
                };
                self.0.entry(key).or_insert((crate::Pixels::new(image), sheet)).clone()
            }
        };
        if let Some(durations) = sheet {
            assets.sheets.insert(id.to_owned(), durations);
        }
        assets.images.insert(id.to_owned(), image);
        Ok(self.0.values().map(|(i, _)| u64::from(i.size().0) * u64::from(i.size().1) * 4).sum())
    }
}
