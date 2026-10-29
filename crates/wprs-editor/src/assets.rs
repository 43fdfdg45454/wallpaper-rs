use crate::layers::{asset_kind, unique};
use crate::ops::{Op, escape, set};
use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use wprs_format::{Limits, MANIFEST, SIGNATURE, sha256_file};
use wprs_scene::{Clip, Lut, MAX_ASSETS, MAX_LUT_BYTES, MAX_SOUND_BYTES, MAX_SOUND_TOTAL};

const DECODED: u64 = 512 << 20;

fn paths<'a>(doc: &'a Value, kind: &str) -> Vec<&'a str> {
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    let found = assets.filter_map(|(_, a)| a["path"].as_str()).filter(|p| asset_kind(p) == kind);
    found.collect::<BTreeSet<_>>().into_iter().collect()
}

fn seconds(file: &Path) -> Result<f32> {
    let size = fs::metadata(file)?.len();
    ensure!(size <= MAX_SOUND_BYTES, "a sound may have at most {} MiB", MAX_SOUND_BYTES >> 20);
    Ok(Clip::probe(&file.to_string_lossy(), &fs::read(file)?).map_err(|e| anyhow!("the sound {e}"))?.seconds)
}

fn pixels(file: &Path) -> Result<u64> {
    let (w, h) = image::image_dimensions(file).context("the picture cannot be read")?;
    Ok(u64::from(w) * u64::from(h) * 4)
}

pub fn check(dir: &Path, doc: &Value, file: &Path) -> Result<()> {
    let kind = asset_kind(&file.to_string_lossy());
    ensure!(kind != "other", "only pictures, videos, sounds, fonts and .cube tables can be imported");
    let size = fs::metadata(file).with_context(|| format!("cannot read {}", file.display()))?.len();
    ensure!(size <= Limits::default().max_file_size, "the file is larger than 1 GiB");
    let count = doc.pointer("/scene/assets").and_then(Value::as_object).map_or(0, |a| a.len());
    ensure!(count < MAX_ASSETS, "a scene has at most {MAX_ASSETS} assets");
    match kind {
        "sound" => {
            let others =
                paths(doc, "sound").iter().map(|p| seconds(&dir.join(p)).unwrap_or(0.0)).sum::<f32>();
            let total = others + seconds(file)?;
            ensure!(total <= MAX_SOUND_TOTAL, "the sounds of a scene may last {MAX_SOUND_TOTAL} s together");
        }
        "image" => {
            let others: u64 = paths(doc, "image").iter().map(|p| pixels(&dir.join(p)).unwrap_or(0)).sum();
            ensure!(
                others + pixels(file)? <= DECODED,
                "the pictures of a scene may take 512 MiB once decoded"
            );
        }
        "lut" => {
            ensure!(size <= MAX_LUT_BYTES as u64, "a color table may have at most 16 MiB");
            Lut::parse(&fs::read(file)?).map_err(|e| anyhow!("{e}"))?;
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
    let name = file.file_name().context("name a file")?.to_string_lossy().replace(['/', '\\', ':'], "_");
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
    Err(anyhow!("assets/ has too many files named {name}"))
}

fn slot(doc: &Value, id: &str) -> Result<String> {
    let pointer = format!("/scene/assets/{}", escape(id));
    ensure!(doc.pointer(&pointer).is_none(), "the asset {id} exists");
    Ok(pointer)
}

pub fn adopt(doc: &Value, id: &str, path: &str) -> Result<Vec<Op>> {
    Ok(set(doc, &slot(doc, id)?, json!({"path": path})))
}

pub fn used(doc: &Value) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = [MANIFEST.to_owned()].into();
    let m = &doc["manifest"];
    used.extend(crate::project::entry(doc).ok());
    used.extend(m["preview"].as_str().map(String::from));
    used.extend(
        m["previews"].as_object().into_iter().flatten().filter_map(|(_, p)| p.as_str().map(String::from)),
    );
    let credits = m["credits"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["paths"].as_array().into_iter().flatten());
    used.extend(credits.filter_map(Value::as_str).filter(|p| !p.ends_with('/')).map(String::from));
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    used.extend(assets.filter_map(|(_, a)| a["path"].as_str().map(String::from)));
    let scripts = doc.pointer("/scene/scripts").and_then(Value::as_array).into_iter().flatten();
    used.extend(scripts.filter_map(|s| s["path"].as_str().map(String::from)));
    let effects = doc.pointer("/scene/effects").and_then(Value::as_object).into_iter().flatten();
    let effects = effects.flat_map(|(_, e)| [&e["descriptor"], &e["wgsl"]]);
    used.extend(effects.filter_map(Value::as_str).map(String::from));
    used
}

pub fn files(dir: &Path) -> Vec<(String, u64)> {
    fn visit(root: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
        for item in fs::read_dir(root.join(prefix)).into_iter().flatten().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            match item.file_type() {
                Ok(t) if t.is_dir() => visit(root, &path, out),
                _ => out.push((path, item.metadata().map_or(0, |m| m.len()))),
            }
        }
    }
    let mut out = Vec::new();
    visit(dir, "", &mut out);
    out.sort();
    out
}

pub fn unused(dir: &Path, doc: &Value) -> Vec<String> {
    let (used, v2) = (used(doc), doc["manifest"]["schema"] == wprs_format::SCHEMA);
    let kept = |p: &String| used.contains(p) || p == SIGNATURE || (v2 && wprs_format::locale_file(p));
    files(dir).into_iter().map(|(path, _)| path).filter(|p| !kept(p)).collect()
}

pub fn prune(dir: &Path, doc: &Value) -> Result<Vec<String>> {
    let gone: Vec<String> = unused(dir, doc).into_iter().filter(|p| p.starts_with("assets/")).collect();
    gone.iter().try_for_each(|p| fs::remove_file(dir.join(p)))?;
    Ok(gone)
}
