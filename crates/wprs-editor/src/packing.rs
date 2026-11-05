use crate::action::AssetEdit;
use crate::layers::asset_kind;
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use image::RgbaImage;
use image::imageops::FilterType;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_ktx2::{Codec, Options, Shape, TARGETS, Target};

pub const QUALITY: u8 = 80;
const LOOK: u32 = 256;

struct Source {
    path: String,
    image: RgbaImage,
    frames: Option<(Vec<RgbaImage>, f32)>,
    bytes: u64,
    options: Options,
}

fn source(dir: &Path, doc: &Value, wanted: &AssetEdit) -> Result<Source> {
    let id = &wanted.asset;
    let asset = doc.pointer(&format!("/scene/assets/{}", escape(id)));
    let asset = asset.ok_or_else(|| anyhow!(said("editor.no_asset", "there is no asset %1", &[id])))?;
    let path = asset["path"].as_str().unwrap_or_default().to_string();
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    ensure!(
        asset_kind(&path) == "image" && ext != "ktx2",
        said("editor.ktx2_picture", "%1 is not a still picture that can become KTX2", &[id])
    );
    let codec = wanted.codec.as_deref().unwrap_or("uastc");
    let codec = Codec::parse(codec)
        .ok_or_else(|| anyhow!(said("editor.ktx2_codec", "%1 is not uastc or etc1s", &[&codec])))?;
    let quality = wanted.quality.unwrap_or(u32::from(QUALITY));
    ensure!(
        (1..=100).contains(&quality),
        said("editor.ktx2_quality", "the quality goes from 1 to 100, not %1", &[&quality])
    );
    let file = dir.join(&path);
    let bytes = fs::metadata(&file)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?
        .len();
    let unreadable =
        |e: &dyn std::fmt::Display| said("editor.unreadable_picture", "the picture cannot be read: %1", &[e]);
    let frames = match asset["animated"] == true {
        true => Some(moving(&fs::read(&file).map_err(|e| unreadable(&e))?).map_err(|e| unreadable(&e))?),
        false => None,
    };
    let image = match &frames {
        Some((list, _)) => list[0].clone(),
        None => crate::pictures::open(&file).map_err(|e| unreadable(&e))?.into_rgba8(),
    };
    let repeat = matches!(asset["wrap"].as_str(), Some("repeat" | "mirror"));
    let srgb = asset["linear"] != true;
    let options = Options { codec, quality: quality as u8, srgb, mips: asset["mipmaps"] != false, repeat };
    Ok(Source { path, image, frames, bytes, options })
}

fn moving(bytes: &[u8]) -> Result<(Vec<RgbaImage>, f32), image::ImageError> {
    let list = wprs_ktx2::moving::frames(bytes, image::Limits::default())?;
    let seconds = list.iter().map(|(_, s)| s).sum::<f32>() / list.len().max(1) as f32;
    let first = list.first().map(|(i, _)| i.dimensions());
    let frames = list.into_iter().map(|(i, _)| i).filter(|i| Some(i.dimensions()) == first).collect();
    Ok((frames, seconds))
}

fn encoded(source: &Source) -> Result<Vec<u8>> {
    let failed = |e: wprs_ktx2::Error| {
        anyhow!(said("editor.ktx2_failed", "the picture cannot become KTX2: %1", &[&e]))
    };
    match &source.frames {
        Some((frames, seconds)) => wprs_ktx2::encode_frames(frames, &source.options, Shape::Frames)
            .map(|b| wprs_ktx2::timed(&b, *seconds))
            .map_err(failed),
        None => wprs_ktx2::encode(&source.image, &source.options).map_err(failed),
    }
}

fn crop(image: &RgbaImage) -> RgbaImage {
    let (w, h) = (image.width().min(LOOK), image.height().min(LOOK));
    let (x, y) = ((image.width() - w) / 2, (image.height() - h) / 2);
    image::imageops::crop_imm(image, x, y, w, h).to_image()
}

fn tag(wanted: &AssetEdit, source: &Source) -> String {
    let codec = if source.options.codec == Codec::Etc1s { "etc1s" } else { "uastc" };
    format!("{}-{codec}-{}", wanted.asset.replace(['/', '\\'], "_"), source.options.quality)
}

type Looks = (String, Vec<(Target, String)>);

fn looks(id: String, original: &RgbaImage, bytes: &[u8], cache: &Path) -> Result<Looks> {
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
        let seen = wprs_ktx2::look(bytes, target)?;
        let first = image::imageops::crop_imm(&seen, 0, 0, info.width, info.height).to_image();
        crop(&first).save(name(target.name()))?;
        made.push((target, name(target.name()).to_string_lossy().into_owned()));
    }
    Ok((name("source").to_string_lossy().into_owned(), made))
}

pub fn trial(dir: &Path, doc: &Value, wanted: &AssetEdit, cache: Option<&Path>) -> Result<Value> {
    let source = source(dir, doc, wanted)?;
    let bytes = encoded(&source)?;
    let info = wprs_ktx2::info(&bytes)?;
    let (original, shown) =
        cache.map(|c| looks(tag(wanted, &source), &source.image, &bytes, c)).transpose()?.unzip();
    let look = |t: Target| shown.iter().flatten().find(|(s, _)| *s == t).map(|(_, p)| p.clone());
    let classes = TARGETS.map(|t| {
        json!({ "target": t.name(), "class": t.class(), "gpu": wprs_ktx2::gpu_bytes(&info, t, source.options.mips),
            "look": look(t) })
    });
    let (w, h) = source.image.dimensions();
    Ok(json!({
        "asset": wanted.asset, "path": source.path, "before": source.bytes, "after": bytes.len(),
        "source": [w, h], "size": [info.width, info.height],
        "levels": info.levels, "codec": info.codec, "quality": source.options.quality, "frames": info.frames,
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
