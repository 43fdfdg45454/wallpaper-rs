use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use std::io::{BufRead, Seek};

fn decoded<R: BufRead + Seek>(
    reader: ImageReader<R>,
    mut limits: Limits,
) -> image::ImageResult<(DynamicImage, Orientation)> {
    let reader = match crate::packed::picture(reader, &limits)? {
        Ok(image) => return Ok((image, Orientation::NoTransforms)),
        Err(reader) => reader,
    };
    let mut reader = reader.with_guessed_format()?;
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    limits.reserve(decoder.total_bytes())?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok((DynamicImage::from_decoder(decoder)?, orientation))
}

pub fn decode_unoriented<R: BufRead + Seek>(
    reader: ImageReader<R>,
    limits: Limits,
) -> image::ImageResult<(DynamicImage, Orientation)> {
    let (image, orientation) = decoded(reader, limits)?;
    Ok((wprs_ktx2::light::shown(image, false), orientation))
}

pub fn decode_limited<R: BufRead + Seek>(
    reader: ImageReader<R>,
    limits: Limits,
) -> image::ImageResult<DynamicImage> {
    let (mut image, orientation) = decode_unoriented(reader, limits)?;
    image.apply_orientation(orientation);
    Ok(image)
}

pub fn decode_asset<R: BufRead + Seek>(
    reader: ImageReader<R>,
    limits: Limits,
    linear: bool,
) -> image::ImageResult<RgbaImage> {
    let (mut image, orientation) = decoded(reader, limits)?;
    image.apply_orientation(orientation);
    Ok(wprs_ktx2::light::eight(&image, linear))
}
