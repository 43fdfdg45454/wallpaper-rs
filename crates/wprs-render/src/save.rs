use crate::parallax::DepthMap;
use crate::{RenderError, Renderer};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use std::io::{BufWriter, Error, ErrorKind};
use std::path::Path;
use wprs_core::{Edit, Fit, UvRect};

pub const FORMATS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "avif"];

impl Renderer {
    pub fn export_edit(
        &self,
        image: &RgbaImage,
        edit: &Edit,
        depth: Option<&DepthMap>,
        size: Option<(u32, u32)>,
    ) -> Result<RgbaImage, RenderError> {
        let mut edit = Edit { parallax: None, ..edit.clone() };
        let (w, h) = edit.geometry.frame(image.dimensions());
        let fit = (self.max_edge() as f32 / w.max(h) as f32).min(1.0);
        let size = size.unwrap_or(((w as f32 * fit) as u32, (h as f32 * fit) as u32));
        let (uv, [x, y, cw, ch]) = (UvRect::compute(Fit::Fill, (w, h), size), edit.geometry.region());
        let [ux, uy, uw, uh] = [uv.offset[0], uv.offset[1], uv.scale[0], uv.scale[1]];
        edit.geometry.crop = Some([x + ux * cw, y + uy * ch, uw * cw, uh * ch]);
        self.read_image(&self.develop(image, &edit, depth, size)?)
    }
}

fn format(out: &Path) -> Option<&'static str> {
    let extension = out.extension()?.to_str()?.to_ascii_lowercase();
    FORMATS.into_iter().find(|f| *f == extension)
}

pub fn save(pixels: &RgbaImage, out: &Path, quality: u8) -> std::io::Result<()> {
    let Some(kind) = format(out) else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "the file name must end in .png, .jpg, .webp or .avif",
        ));
    };
    let (w, h, quality) = (pixels.width(), pixels.height(), quality.clamp(1, 100));
    let rgb = image::DynamicImage::ImageRgba8(pixels.clone()).into_rgb8();
    let file = BufWriter::new(std::fs::File::create(out)?);
    let written = match kind {
        "png" => image::codecs::png::PngEncoder::new(file).write_image(&rgb, w, h, ExtendedColorType::Rgb8),
        "webp" => image::codecs::webp::WebPEncoder::new_lossless(file).write_image(
            pixels,
            w,
            h,
            ExtendedColorType::Rgba8,
        ),
        #[cfg(feature = "avif")]
        "avif" => image::codecs::avif::AvifEncoder::new_with_speed_quality(file, 6, quality).write_image(
            &rgb,
            w,
            h,
            ExtendedColorType::Rgb8,
        ),
        #[cfg(not(feature = "avif"))]
        "avif" => return Err(Error::new(ErrorKind::Unsupported, "this build cannot write AVIF files")),
        _ => image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality).write_image(
            &rgb,
            w,
            h,
            ExtendedColorType::Rgb8,
        ),
    };
    written.map_err(Error::other)
}
