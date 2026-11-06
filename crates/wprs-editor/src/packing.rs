use crate::action::AssetEdit;
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use image::RgbaImage;
use image::imageops::FilterType;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_ktx2::{Codec, TARGETS, Target};

mod source;

use source::{Source, encoded, source};

pub const QUALITY: u8 = 80;
const LOOK: u32 = 256;

fn crop(image: &RgbaImage) -> RgbaImage {
    let (w, h) = (image.width().min(LOOK), image.height().min(LOOK));
    let (x, y) = ((image.width() - w) / 2, (image.height() - h) / 2);
    image::imageops::crop_imm(image, x, y, w, h).to_image()
}

fn tag(wanted: &AssetEdit, source: &Source) -> String {
    let codec = if source.options.codec == Codec::Etc1s { "etc1s" } else { "uastc" };
    format!("{}-{codec}-{}", wanted.asset.replace(['/', '\\'], "_"), source.options.quality)
}

type Looks = (String, Vec<(Target, String, &'static str)>);

pub type Gpu<'a> = &'a dyn Fn(&[u8], Target) -> Option<RgbaImage>;

fn looks(
    id: String,
    original: &RgbaImage,
    bytes: &[u8],
    (cache, gpu): (&Path, Option<Gpu>),
) -> Result<Looks> {
    fs::create_dir_all(cache)?;
    let name = |what: &str| cache.join(format!("{id}-{what}.png"));
    let info = wprs_ktx2::info(bytes)?;
    let fitted = match original.dimensions() == (info.width, info.height) {
        true => original.clone(),
        false => image::imageops::resize(original, info.width, info.height, FilterType::Triangle),
    };
    crop(&fitted).save(name("source"))?;
    let mut made = Vec::new();
    for target in TARGETS {
        let drawn = gpu.and_then(|g| g(bytes, target));
        let by = match (drawn.is_some(), target.compressed()) {
            (true, _) => "gpu",
            (_, true) => "cpu",
            _ => "plain",
        };
        let whole = info.hdr || wprs_ktx2::plain(&info, target);
        let decoded = || {
            if whole { wprs_ktx2::rgba(bytes, wprs_ktx2::MAX_SIDE) } else { wprs_ktx2::look(bytes, target) }
        };
        let seen = drawn.map_or_else(decoded, Ok)?;
        let first = image::imageops::crop_imm(&seen, 0, 0, info.width, info.height).to_image();
        crop(&first).save(name(target.name()))?;
        made.push((target, name(target.name()).to_string_lossy().into_owned(), by));
    }
    Ok((name("source").to_string_lossy().into_owned(), made))
}

pub fn trial(
    dir: &Path,
    doc: &Value,
    wanted: &AssetEdit,
    cache: Option<(&Path, Option<Gpu>)>,
) -> Result<Value> {
    let source = source(dir, doc, wanted)?;
    let bytes = encoded(&source)?;
    let info = wprs_ktx2::info(&bytes)?;
    let (original, shown) =
        cache.map(|c| looks(tag(wanted, &source), &source.image, &bytes, c)).transpose()?.unzip();
    let look = |t: Target| shown.iter().flatten().find(|(s, ..)| *s == t).map(|(_, p, by)| (p.clone(), *by));
    let classes = TARGETS.map(|t| {
        json!({ "target": t.name(), "class": t.class(), "gpu": wprs_ktx2::gpu_bytes(&info, t, source.options.mips),
            "plain": wprs_ktx2::plain(&info, t),
            "look": look(t).map(|l| l.0), "drawn": look(t).map(|l| l.1) })
    });
    let (w, h) = source.image.dimensions();
    Ok(json!({
        "asset": wanted.asset, "path": source.path, "before": source.bytes, "after": bytes.len(),
        "source": [w, h], "size": [info.width, info.height],
        "levels": info.levels, "codec": info.codec, "quality": source.options.quality, "frames": info.frames,
        "hdr": info.hdr, "cube": info.cube,
        "original": original, "classes": classes,
    }))
}

fn written(dir: &Path, stem: &str, bytes: &[u8]) -> Result<String> {
    fs::create_dir_all(dir.join("assets"))?;
    for n in 1..10_000 {
        let path = if n == 1 { format!("assets/{stem}.ktx2") } else { format!("assets/{stem}-{n}.ktx2") };
        wprs_format::check_path(&path)?;
        let target = dir.join(&path);
        match fs::read(&target) {
            Ok(old) if old == bytes => return Ok(path),
            Ok(_) => {}
            Err(_) => {
                let part = dir.join("assets").join(".ktx2.part");
                fs::write(&part, bytes).and_then(|_| fs::rename(&part, &target))?;
                return Ok(path);
            }
        }
    }
    Err(anyhow!(said("editor.assets_too_many", "assets/ has too many files named %1", &[&stem])))
}

pub fn store(dir: &Path, doc: &Value, wanted: &AssetEdit) -> Result<String> {
    let source = source(dir, doc, wanted)?;
    let bytes = encoded(&source)?;
    let name = source.path.rsplit('/').next().unwrap_or("picture");
    written(dir, name.rsplit_once('.').map_or(name, |(s, _)| s), &bytes)
}

pub fn adopt(doc: &Value, id: &str, path: &str) -> Vec<Op> {
    set(doc, &format!("/scene/assets/{}/path", escape(id)), json!(path))
}

pub(crate) fn act(dir: &Path, doc: &Value, wanted: &AssetEdit) -> Result<Vec<Op>> {
    if wanted.cover.is_some() || wanted.placeholder.is_some() || wanted.field.is_some() {
        return crate::covers::act(doc, wanted);
    }
    ensure!(
        wanted.ktx2,
        said("editor.asset_change", "say what to change in %1, such as ktx2", &[&wanted.asset])
    );
    store(dir, doc, wanted).map(|path| adopt(doc, &wanted.asset, &path))
}
