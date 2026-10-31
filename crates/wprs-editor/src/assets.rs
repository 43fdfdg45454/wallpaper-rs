pub use crate::contents::{files, prune, unused, used};
use crate::layers::{asset_kind, unique};
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use wprs_core::message::said;
use wprs_format::{Limits, sha256_file};
use wprs_scene::{
    Clip, Lut, MAX_ASSETS, MAX_LUT_BYTES, MAX_SOUND_SECONDS, MAX_SOUND_TOTAL, MAX_STREAM_BYTES,
};

const DECODED: u64 = 512 << 20;

fn paths<'a>(doc: &'a Value, kind: &str) -> Vec<&'a str> {
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    let found = assets.filter_map(|(_, a)| a["path"].as_str()).filter(|p| asset_kind(p) == kind);
    found.collect::<BTreeSet<_>>().into_iter().collect()
}

fn seconds(file: &Path) -> Result<f32> {
    let size = fs::metadata(file)?.len();
    ensure!(
        size <= MAX_STREAM_BYTES,
        said("editor.sound_may_most", "a sound may have at most %1 MiB", &[&(MAX_STREAM_BYTES >> 20)])
    );
    Ok(Clip::read(&file.to_string_lossy(), &fs::read(file)?)
        .map_err(|e| anyhow!(said("editor.sound", "the sound %1", &[&e])))?
        .seconds)
}

fn pixels(file: &Path) -> Result<u64> {
    let (w, h) = image::image_dimensions(file)
        .map_err(|e| said("editor.unreadable_picture", "the picture cannot be read: %1", &[&e]))?;
    Ok(u64::from(w) * u64::from(h) * 4)
}

pub fn check(dir: &Path, doc: &Value, file: &Path) -> Result<()> {
    let kind = asset_kind(&file.to_string_lossy());
    ensure!(
        kind != "other",
        said(
            "editor.only_pictures_videos",
            "only pictures, videos, sounds, fonts and .cube tables can be imported",
            &[]
        )
    );
    let size = fs::metadata(file)
        .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?
        .len();
    ensure!(
        size <= Limits::default().max_file_size,
        said("editor.file_larger_than", "the file is larger than 1 GiB", &[])
    );
    let count = doc.pointer("/scene/assets").and_then(Value::as_object).map_or(0, |a| a.len());
    ensure!(
        count < MAX_ASSETS,
        said("editor.scene_most_assets", "a scene has at most %1 assets", &[&MAX_ASSETS])
    );
    match kind {
        "sound" => {
            let held = |s: f32| if s > MAX_SOUND_SECONDS { 0.0 } else { s };
            let others =
                paths(doc, "sound").iter().map(|p| held(seconds(&dir.join(p)).unwrap_or(0.0))).sum::<f32>();
            let total = others + held(seconds(file)?);
            ensure!(
                total <= MAX_SOUND_TOTAL,
                said(
                    "editor.sounds_scene_may",
                    "the sounds of a scene may last %1 s together",
                    &[&MAX_SOUND_TOTAL]
                )
            );
        }
        "image" => {
            let others: u64 = paths(doc, "image").iter().map(|p| pixels(&dir.join(p)).unwrap_or(0)).sum();
            ensure!(
                others + pixels(file)? <= DECODED,
                said(
                    "editor.pictures_scene_may",
                    "the pictures of a scene may take 512 MiB once decoded",
                    &[]
                )
            );
        }
        "lut" => {
            ensure!(
                size <= MAX_LUT_BYTES as u64,
                said("editor.color_table_may", "a color table may have at most 16 MiB", &[])
            );
            Lut::parse(&fs::read(file)?).map_err(|e| e.message())?;
        }
        _ => {}
    }
    Ok(())
}

fn copy(file: &Path, target: &Path) -> Result<()> {
    let part = target.with_file_name(format!(".{}.part", target.file_name().unwrap_or_default().display()));
    let copied = fs::File::open(file).and_then(|mut from| {
        let mut to = fs::File::create(&part)?;
        io::copy(&mut from, &mut to)?;
        to.sync_all()?;
        fs::rename(&part, target)
    });
    if copied.is_err() {
        fs::remove_file(&part).ok();
    }
    Ok(copied?)
}

pub fn import(dir: &Path, doc: &Value, file: &Path, id: Option<&str>) -> Result<(String, String)> {
    check(dir, doc, file)?;
    let name = file
        .file_name()
        .ok_or_else(|| said("editor.name_a_file", "name a file", &[]))?
        .to_string_lossy()
        .replace(['/', '\\', ':'], "_");
    let id =
        id.map_or_else(|| unique(doc, name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s)), String::from);
    slot(doc, &id)?;
    Ok((id, place(dir, &name, file)?))
}

pub fn place(dir: &Path, name: &str, file: &Path) -> Result<String> {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let (size, mut digest) = (fs::metadata(file)?.len(), None);
    fs::create_dir_all(dir.join("assets"))?;
    for n in 1..10_000 {
        let path = if n == 1 { format!("assets/{name}") } else { format!("assets/{stem}-{n}.{ext}") };
        wprs_format::check_path(&path)?;
        let target = dir.join(&path);
        match fs::metadata(&target) {
            Ok(old) if old.len() == size => {
                let source = match &digest {
                    Some(d) => d,
                    None => digest.insert(sha256_file(file)?),
                };
                if sha256_file(&target)? == *source {
                    return Ok(path);
                }
            }
            Ok(_) => {}
            Err(_) => return copy(file, &target).map(|_| path),
        }
    }
    Err(anyhow!(said("editor.assets_too_many", "assets/ has too many files named %1", &[&name])))
}

fn slot(doc: &Value, id: &str) -> Result<String> {
    let pointer = format!("/scene/assets/{}", escape(id));
    ensure!(doc.pointer(&pointer).is_none(), said("editor.asset_exists", "the asset %1 exists", &[&id]));
    Ok(pointer)
}

pub fn adopt(doc: &Value, id: &str, path: &str) -> Result<Vec<Op>> {
    Ok(set(doc, &slot(doc, id)?, json!({"path": path})))
}
