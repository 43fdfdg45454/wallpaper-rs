use crate::Error;
use image::{ImageReader, Limits, RgbaImage};
use std::io::Cursor;

pub(super) fn decode(bytes: Vec<u8>) -> Result<RgbaImage, Error> {
    drawn(bytes, false)
}

pub(super) fn drawn(bytes: Vec<u8>, linear: bool) -> Result<RgbaImage, Error> {
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(1 << 14), Some(1 << 14), Some(1 << 28));
    Ok(wprs_render::decode_asset(ImageReader::new(Cursor::new(bytes)), limits, linear)?)
}
