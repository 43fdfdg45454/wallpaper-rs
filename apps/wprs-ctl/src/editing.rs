use crate::usage;
use anyhow::{Context, Result, ensure};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};
use std::io::BufWriter;
use std::path::Path;
use wprs_client::Client;
use wprs_core::edit::{self, Action, Edit, Source};
use wprs_core::{Fit, UvRect};
use wprs_render::Renderer;

fn text(e: String) -> anyhow::Error {
    anyhow::Error::msg(e)
}

fn refused(e: edit::Error) -> anyhow::Error {
    match e {
        edit::Error::Usage(message) => usage(message),
        edit::Error::Invalid(message) => text(message),
    }
}

pub fn dimensions(size: &str) -> Result<(u32, u32)> {
    let parsed =
        size.split_once('x').and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)));
    parsed.ok_or_else(|| usage(format!("{size} is not WIDTHxHEIGHT")))
}

fn source(path: &Path) -> Result<Source> {
    let (w, h) =
        wprs_render::image_size(path).with_context(|| format!("{} is not a picture", path.display()))?;
    Ok(Source::read(path, [w, h])?)
}

pub fn open(path: &Path) -> Result<Edit> {
    ensure!(path.is_file(), "cannot find {}", path.display());
    let Some((_, edit)) = edit::load(path, &edit::dir()).map_err(text)? else {
        return Ok(Edit { source: source(path)?, ..Edit::default() });
    };
    let (s, bytes) = (&edit.source, std::fs::metadata(path)?.len());
    let pixels = wprs_render::image_size(path).map(|(w, h)| [w, h]).unwrap_or_default();
    if s.bytes != 0 && s.bytes != bytes || s.pixels != [0; 2] && s.pixels != pixels {
        eprintln!("warning: {} changed after it was edited", path.display());
    }
    Ok(edit)
}

pub fn change(path: &Path, action: Action) -> Result<()> {
    let mut edit = open(path)?;
    edit.act(action).map_err(refused)?;
    edit::commit(&edit, path, &edit::dir()).map(drop).map_err(text)
}

pub fn apply(path: &Path, file: &str) -> Result<()> {
    let json = match file {
        "-" => std::io::read_to_string(std::io::stdin())?,
        file => std::fs::read_to_string(file).with_context(|| format!("cannot read {file}"))?,
    };
    ensure!(path.is_file(), "cannot find {}", path.display());
    let mut edit = Edit::parse(&json).map_err(text)?;
    if edit.source == Source::default() {
        edit.source = source(path)?;
    }
    edit::save(&edit, path, &edit::dir()).map(drop).map_err(text)
}

pub fn preview(path: &Path, changes: Vec<String>, stop: bool) -> Result<()> {
    let path = std::fs::canonicalize(path).with_context(|| format!("cannot find {}", path.display()))?;
    let mut edit = if stop { None } else { Some(open(&path)?) };
    edit.as_mut().map_or(Ok(()), |edit| edit.act(Action::Set { changes }).map_err(refused))?;
    let (path, json) =
        (path.to_str().context("the path is not UTF-8")?, edit.map(|e| e.to_json()).unwrap_or_default());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async { Ok(Client::connect().await?.daemon.preview_edit(path, &json).await?) })
}

pub fn export(path: &Path, out: &Path, quality: u8, size: Option<&str>) -> Result<()> {
    let mut edit = open(path)?;
    let renderer = Renderer::new()?;
    let image = wprs_render::decode_image(path, renderer.max_edge())?;
    let (w, h) = edit.geometry.frame(image.dimensions());
    let fit = (renderer.max_edge() as f32 / w.max(h) as f32).min(1.0);
    let size =
        size.map(dimensions).transpose()?.unwrap_or(((w as f32 * fit) as u32, (h as f32 * fit) as u32));
    let (uv, [x, y, cw, ch]) = (UvRect::compute(Fit::Fill, (w, h), size), edit.geometry.region());
    let [ux, uy, uw, uh] = [uv.offset[0], uv.offset[1], uv.scale[0], uv.scale[1]];
    edit.geometry.crop = Some([x + ux * cw, y + uy * ch, uw * cw, uh * ch]);
    let pixels = renderer.read_image(&renderer.develop(&image, &edit, size)?)?;
    match out.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => pixels.save_with_format(out, ImageFormat::Png)?,
        Some("jpg" | "jpeg") => {
            let file = BufWriter::new(std::fs::File::create(out)?);
            JpegEncoder::new_with_quality(file, quality)
                .encode_image(&DynamicImage::ImageRgba8(pixels).into_rgb8())?
        }
        _ => return Err(usage("the file name must end in .png, .jpg or .jpeg")),
    }
    Ok(())
}
