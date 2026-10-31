use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use std::path::{Path, PathBuf};

pub fn thumbnails(
    mut image: DynamicImage,
    thumbs: &[(u32, PathBuf)],
    written: &mut Vec<u32>,
) -> image::ImageResult<()> {
    let mut thumbs = thumbs.to_vec();
    thumbs.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    for (size, dest) in thumbs {
        if image.width().max(image.height()) > size {
            image = image.resize(size, size, FilterType::Triangle);
        }
        let dir = dest.parent().unwrap_or(Path::new("."));
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        image.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut file, 85))?;
        file.persist(&dest).map_err(|e| e.error)?;
        written.push(size);
    }
    Ok(())
}
