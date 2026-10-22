use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use std::io::{BufRead, Cursor, Seek, Write};
use std::path::Path;
use wprs_format::{Kind as PackageKind, Package};
use wprs_library::Meta;

pub struct Picture {
    pub meta: Meta,
    pub image: Option<DynamicImage>,
    pub failure: Option<String>,
}

pub enum Problem {
    Missing(String),
    Decode(String),
}

impl<E: std::error::Error + 'static> From<E> for Problem {
    fn from(e: E) -> Self {
        let text = e.to_string();
        let source = std::iter::successors(Some(&e as &(dyn std::error::Error + 'static)), |e| e.source());
        let io = source.filter_map(|e| e.downcast_ref::<std::io::Error>()).next().map(std::io::Error::kind);
        match io {
            Some(std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) => {
                Problem::Missing(text)
            }
            _ => Problem::Decode(text),
        }
    }
}

impl Picture {
    pub fn of(image: DynamicImage) -> Picture {
        let meta = Meta { width: Some(image.width()), height: Some(image.height()), ..Meta::default() };
        Picture { meta, image: Some(image), failure: None }
    }
}

pub fn decode<R: BufRead + Seek>(reader: ImageReader<R>) -> Result<DynamicImage, Problem> {
    let mut reader = reader.with_guessed_format()?;
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(1 << 15), Some(1 << 15), Some(1 << 30));
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn bytes(data: Vec<u8>) -> Result<DynamicImage, Problem> {
    decode(ImageReader::new(Cursor::new(data)))
}

pub fn video(path: &Path) -> Result<Picture, Problem> {
    let still = wprs_video::still(path).map_err(|e| match e {
        wprs_video::Error::NotLocal(_) => Problem::Missing(e.to_string()),
        e => Problem::Decode(e.to_string()),
    })?;
    let (duration_ms, audio) = (still.duration_ms, Some(still.audio));
    let frame = RgbaImage::from_raw(still.width, still.height, still.rgba).map(DynamicImage::ImageRgba8);
    let picture = Picture::of(frame.ok_or_else(|| Problem::Decode("the video frame is incomplete".into()))?);
    Ok(Picture { meta: Meta { duration_ms, audio, ..picture.meta }, ..picture })
}

pub fn package(path: &Path) -> Result<Picture, Problem> {
    let mut package: Package<_> = wprs_format::open(path, &wprs_format::Limits::default())?;
    let manifest = package.manifest.clone();
    let preview = manifest.preview.as_ref().map(|p| bytes(package.read_file(p)?)).transpose()?;
    let entry = package.read_file(&manifest.entry);
    let entry = match manifest.kind {
        PackageKind::Scene => {
            let [width, height] = package.scene()?.canvas.size.map(Some);
            let failure = preview.is_none().then(|| "no preview".to_owned());
            return Ok(Picture { meta: Meta { width, height, ..Meta::default() }, image: preview, failure });
        }
        PackageKind::Image => Picture::of(bytes(entry?)?),
        PackageKind::Video => {
            let extension = Path::new(&manifest.entry).extension().unwrap_or_default().to_string_lossy();
            let mut file = tempfile::Builder::new().suffix(&format!(".{extension}")).tempfile()?;
            file.write_all(&entry?)?;
            video(file.path())?
        }
    };
    Ok(Picture { image: preview.or(entry.image), ..entry })
}

pub fn color(image: &DynamicImage) -> u32 {
    let small = image.thumbnail(32, 32).to_rgb8();
    let mut bins = vec![(0u32, [0u32; 3]); 4096];
    for pixel in small.pixels() {
        let [r, g, b] = pixel.0.map(u32::from);
        let bin = &mut bins[(((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4)) as usize];
        bin.0 += 1;
        (0..3).for_each(|i| bin.1[i] += u32::from(pixel.0[i]));
    }
    let (count, sum) = bins.into_iter().max_by_key(|(count, _)| *count).unwrap_or_default();
    let [r, g, b] = sum.map(|c| c / count.max(1));
    (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_color_is_the_largest_bin() {
        let mut image = RgbaImage::from_pixel(64, 64, image::Rgba([0x33, 0x66, 0xff, 255]));
        (0..64).for_each(|x| image.put_pixel(x, 0, image::Rgba([255, 0, 0, 255])));
        assert_eq!(color(&DynamicImage::ImageRgba8(image)), 0x3366ff);
    }
}
