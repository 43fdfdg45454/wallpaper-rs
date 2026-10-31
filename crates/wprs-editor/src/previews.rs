use crate::assets::place;
use crate::ops::{Op, set};
use anyhow::{Result, bail, ensure};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_format::SCHEMA;
use wprs_format::media::{preview, probe};

const MOST: usize = 1 << 20;

fn shrink(data: &[u8]) -> Result<Vec<u8>> {
    let picture = image::load_from_memory(data)
        .map_err(|e| said("editor.unreadable_picture", "the picture cannot be read: %1", &[&e]))?;
    for side in [1024, 768, 512] {
        let fitted = match picture.width().max(picture.height()) > side {
            true => picture.resize(side, side, FilterType::Triangle),
            false => picture.clone(),
        };
        let pixels = fitted.to_rgb8();
        for quality in [85, 75, 65] {
            let mut out = Vec::new();
            JpegEncoder::new_with_quality(&mut out, quality).encode_image(&pixels)?;
            if out.len() <= MOST {
                return Ok(out);
            }
        }
    }
    bail!(said(
        "editor.picture_does_not",
        "the picture does not fit in 1 MiB even as a jpeg of 512 pixels",
        &[]
    ))
}

pub fn store(dir: &Path, doc: &Value, file: &Path, animated: bool) -> Result<String> {
    field(doc, animated)?;
    let data = fs::read(file)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?;
    let found = probe(&data)
        .ok_or_else(|| said("editor.preview_kind", "choose a png, jpeg, webp or gif picture", &[]))?;
    let name = |ext: &str| format!("preview{}.{ext}", if animated { "-animated" } else { "" });
    let ext = if found.format == "jpeg" { "jpg" } else { found.format };
    let (most, kinds) = if animated { (8 << 20, "gif webp") } else { (MOST, "png jpeg webp") };
    let usable = kinds.split(' ').any(|k| k == found.format) && data.len() <= most;
    if usable && preview(animated, &name(ext), &data).is_ok() {
        return place(dir, &name(ext), file);
    }
    ensure!(
        !animated,
        said(
            "editor.animated_preview_gif",
            "an animated preview is a gif or webp of at most 640 pixels a side, 15 s and 8 MiB",
            &[]
        )
    );
    let part = dir.join("assets").join(".preview.part");
    fs::create_dir_all(dir.join("assets"))?;
    fs::write(&part, shrink(&data)?)?;
    let placed = place(dir, &name("jpg"), &part);
    fs::remove_file(&part).ok();
    placed
}

fn field(doc: &Value, animated: bool) -> Result<&'static str> {
    Ok(match (animated, doc["manifest"]["schema"] == SCHEMA) {
        (false, false) => "preview",
        (true, false) => bail!(said(
            "editor.animated_preview_needs",
            "an animated preview needs %1; upgrade the manifest first",
            &[&SCHEMA]
        )),
        (false, true) => "previews/thumbnail",
        (true, true) => "previews/animated",
    })
}

pub fn adopt(doc: &Value, animated: bool, path: &str) -> Result<Vec<Op>> {
    Ok(set(doc, &format!("/manifest/{}", field(doc, animated)?), json!(path)))
}
