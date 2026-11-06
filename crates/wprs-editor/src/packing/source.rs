use crate::action::AssetEdit;
use crate::layers::asset_kind;
use crate::ops::escape;
use anyhow::{Result, anyhow, ensure};
use image::{DynamicImage, GenericImageView, Rgba32FImage, RgbaImage};
use serde_json::Value;
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_ktx2::{Codec, Options, Shape};

pub(super) struct Source {
    pub(super) path: String,
    pub(super) image: RgbaImage,
    pub(super) frames: Option<(Vec<RgbaImage>, Vec<f32>)>,
    pub(super) faces: Option<Vec<RgbaImage>>,
    pub(super) light: Option<Vec<Rgba32FImage>>,
    pub(super) bytes: u64,
    pub(super) options: Options,
}

fn unreadable(e: &dyn std::fmt::Display) -> anyhow::Error {
    anyhow!(said("editor.unreadable_picture", "the picture cannot be read: %1", &[e]))
}

fn entry<'a>(doc: &'a Value, id: &str) -> Result<(&'a Value, String)> {
    let asset = doc.pointer(&format!("/scene/assets/{}", escape(id)));
    let asset = asset.ok_or_else(|| anyhow!(said("editor.no_asset", "there is no asset %1", &[&id])))?;
    let path = asset["path"].as_str().unwrap_or_default().to_string();
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    ensure!(
        asset_kind(&path) == "image" && ext != "ktx2",
        said("editor.ktx2_picture", "%1 is not a still picture that can become KTX2", &[&id])
    );
    Ok((asset, path))
}

fn bright(picture: &DynamicImage) -> bool {
    matches!(picture, DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_))
}

fn faces(dir: &Path, doc: &Value, wanted: &AssetEdit) -> Result<Vec<DynamicImage>> {
    let open = |id: &String| -> Result<DynamicImage> {
        let path = entry(doc, id)?.1;
        crate::pictures::open(dir.join(path)).map_err(|e| unreadable(&e))
    };
    let list: Vec<DynamicImage> = wanted.faces.iter().map(open).collect::<Result<_>>()?;
    let size = list.first().map(GenericImageView::dimensions);
    let square = size.is_some_and(|(w, h)| w == h) && list.iter().all(|f| Some(f.dimensions()) == size);
    ensure!(
        list.len() == 6 && square,
        said("editor.ktx2_faces", "a cube map takes six square pictures of one size, not %1", &[&list.len()])
    );
    Ok(list)
}

pub(super) fn source(dir: &Path, doc: &Value, wanted: &AssetEdit) -> Result<Source> {
    let (asset, path) = entry(doc, &wanted.asset)?;
    let codec = wanted.codec.as_deref().unwrap_or("uastc");
    let codec = Codec::parse(codec)
        .ok_or_else(|| anyhow!(said("editor.ktx2_codec", "%1 is not uastc or etc1s", &[&codec])))?;
    let quality = wanted.quality.unwrap_or(u32::from(super::QUALITY));
    ensure!(
        (1..=100).contains(&quality),
        said("editor.ktx2_quality", "the quality goes from 1 to 100, not %1", &[&quality])
    );
    let shaped = wanted.cube || !wanted.faces.is_empty();
    ensure!(
        !(wanted.cube && !wanted.faces.is_empty()) && !(shaped && asset["animated"] == true),
        said("editor.ktx2_shape", "a cube map comes from one panorama or six faces, never an animation", &[])
    );
    let file = dir.join(&path);
    let bytes = fs::metadata(&file)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?
        .len();
    let frames = match asset["animated"] == true {
        true => Some(moving(&fs::read(&file).map_err(|e| unreadable(&e))?).map_err(|e| unreadable(&e))?),
        false => None,
    };
    let pictures = match (&frames, wanted.faces.is_empty()) {
        (Some(_), _) => Vec::new(),
        (None, false) => faces(dir, doc, wanted)?,
        (None, true) => vec![crate::pictures::open(&file).map_err(|e| unreadable(&e))?],
    };
    let light = pictures.iter().any(bright);
    let wide: Vec<Rgba32FImage> = pictures.iter().map(|p| p.to_rgba32f()).collect();
    let wide = match wanted.cube {
        true => wprs_ktx2::faces(&wide[0]).map_err(|e| unreadable(&e))?,
        false => wide,
    };
    let plain: Vec<RgbaImage> = match wanted.cube && !light {
        true => wprs_ktx2::plain_faces(&pictures[0].to_rgba8()).map_err(|e| unreadable(&e))?,
        false => pictures.iter().map(DynamicImage::to_rgba8).collect(),
    };
    let image = match &frames {
        Some((list, _)) => list[0].clone(),
        None => plain.first().cloned().unwrap_or_default(),
    };
    let repeat = matches!(asset["wrap"].as_str(), Some("repeat" | "mirror"));
    let srgb = asset["linear"] != true;
    let options = Options { codec, quality: quality as u8, srgb, mips: asset["mipmaps"] != false, repeat };
    let faces = (shaped && !light).then_some(plain);
    let light = light.then_some(wide);
    Ok(Source { path, image, frames, faces, light, bytes, options })
}

fn moving(bytes: &[u8]) -> Result<(Vec<RgbaImage>, Vec<f32>), image::ImageError> {
    let list = wprs_ktx2::moving::frames(bytes, image::Limits::default())?;
    let first = list.first().map(|(i, _)| i.dimensions());
    Ok(list.into_iter().filter(|(i, _)| Some(i.dimensions()) == first).unzip())
}

pub(super) fn encoded(source: &Source) -> Result<Vec<u8>> {
    let failed = |e: wprs_ktx2::Error| {
        anyhow!(said("editor.ktx2_failed", "the picture cannot become KTX2: %1", &[&e]))
    };
    let o = &source.options;
    match (&source.light, &source.faces, &source.frames) {
        (Some(layers), ..) => wprs_ktx2::encode_light(layers, o).map_err(failed),
        (_, Some(faces), _) => wprs_ktx2::encode_frames(faces, o, Shape::Cube).map_err(failed),
        (.., Some((frames, seconds))) => wprs_ktx2::encode_frames(frames, o, Shape::Frames)
            .map(|b| wprs_ktx2::paced(&b, seconds))
            .map_err(failed),
        _ => wprs_ktx2::encode(&source.image, o).map_err(failed),
    }
}
