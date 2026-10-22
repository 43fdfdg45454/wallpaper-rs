use crate::usage;
use anyhow::{Context, Result, ensure};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};
use std::io::BufWriter;
use std::path::Path;
use wprs_client::Client;
use wprs_core::edit::{self, Edit, Kind, Mask, Op, Source};
use wprs_core::{Fit, UvRect};
use wprs_render::Renderer;

fn text(e: String) -> anyhow::Error {
    anyhow::Error::msg(e)
}

pub fn numbers<const N: usize>(list: &str) -> Result<[f32; N]> {
    let parsed: Result<Vec<f32>, _> = list.split(',').map(|n| n.trim().parse::<f32>()).collect();
    let parsed = parsed.map_err(|_| usage(format!("{list} is not a list of numbers")))?;
    parsed.try_into().map_err(|_| usage(format!("{list} does not have {N} numbers")))
}

pub fn dimensions(size: &str) -> Result<(u32, u32)> {
    let parsed =
        size.split_once('x').and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)));
    parsed.ok_or_else(|| usage(format!("{size} is not WIDTHxHEIGHT")))
}

fn source(path: &Path) -> Result<Source> {
    let (w, h) =
        wprs_render::image_size(path).with_context(|| format!("{} is not a picture", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (bytes, sha256) = (std::fs::metadata(path)?.len(), wprs_format::sha256_file(path)?);
    Ok(Source { name, bytes, sha256, pixels: [w, h] })
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

pub fn change(path: &Path, apply: impl FnOnce(&mut Edit) -> Result<()>) -> Result<()> {
    let mut edit = open(path)?;
    apply(&mut edit)?;
    edit::save(&edit, path, &edit::dir()).map(drop).map_err(text)
}

pub fn set(edit: &mut Edit, changes: &[String]) -> Result<()> {
    changes.iter().try_for_each(|change| {
        let (key, value) =
            change.split_once('=').ok_or_else(|| usage(format!("{change} is not KEY=VALUE")))?;
        edit.set(key, value).map_err(text)
    })
}

pub fn upsert(
    edit: &mut Edit,
    id: &str,
    kind: Kind,
    mask: Option<String>,
    opacity: Option<f32>,
) -> Result<()> {
    match edit.ops.iter_mut().find(|op| op.id == id) {
        Some(op) => (op.kind, op.mask, op.opacity) = (kind, mask.or(op.mask.take()), opacity.or(op.opacity)),
        None => edit.ops.push(Op { id: id.into(), enabled: true, mask, opacity, kind }),
    }
    Ok(())
}

pub fn named(edit: &mut Edit, name: &str, spec: &str, invert: bool) -> Result<()> {
    let (kind, values) = spec.split_once(':').ok_or_else(|| usage(format!("{spec} is not a mask")))?;
    let short = values.split(',').count() < 3 || kind == "radial" && values.split(',').count() == 4;
    let full =
        if short { format!("{values},{}", if kind == "radial" { 0.5 } else { 0.1 }) } else { values.into() };
    let mask = match kind {
        "linear" => {
            numbers(values).map(|[x0, y0, x1, y1]| Mask::Linear { from: [x0, y0], to: [x1, y1], invert })
        }
        "radial" => numbers(&full).map(|[x, y, rx, ry, feather]| Mask::Radial {
            center: [x, y],
            radius: [rx, ry],
            feather,
            invert,
        }),
        "luminance" => numbers(&full).map(|[lo, hi, soft]| Mask::Luminance { range: [lo, hi], soft, invert }),
        _ => Err(usage(format!("{kind} is not linear, radial or luminance"))),
    };
    edit.masks.insert(name.into(), mask?);
    Ok(())
}

pub fn mask(edit: &mut Edit, id: &str, spec: &str) -> Result<String> {
    if spec.contains(':') {
        named(edit, id, spec, false)?;
        return Ok(id.into());
    }
    Ok(spec.into())
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

pub fn preview(path: &Path, changes: &[String], stop: bool) -> Result<()> {
    let path = std::fs::canonicalize(path).with_context(|| format!("cannot find {}", path.display()))?;
    let mut edit = if stop { None } else { Some(open(&path)?) };
    edit.as_mut().map_or(Ok(()), |edit| set(edit, changes))?;
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
