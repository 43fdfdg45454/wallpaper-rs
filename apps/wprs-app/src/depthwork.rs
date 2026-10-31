use crate::bus::url;
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use wprs_core::edit::{self, Depth, Edit, Source};
use wprs_depth::models::assets;
use wprs_depth::{MODEL, Part, Places, beside, read, sha256, write};

pub enum Work {
    Open,
    Download,
    Generate,
    Adopt(PathBuf),
    Keep,
    Install(PathBuf),
}

#[derive(Default)]
pub struct Shown {
    pub assets: String,
    pub map: String,
    pub sha: String,
    pub ready: bool,
    pub allowed: bool,
}

pub struct Target {
    pub image: PathBuf,
    pub sha: String,
    pub depth: Depth,
    pub device: wprs_depth::Device,
}

pub type Report<'a> = &'a mut dyn FnMut(&str, f64);

pub fn sha(image: &Path, edit: &Edit) -> Result<String> {
    match edit.source.sha256.as_str() {
        "" => Ok(Source::read(image, [0; 2])?.sha256),
        known => Ok(known.to_owned()),
    }
}

pub fn run(
    places: &Places,
    mut target: Target,
    work: Work,
    stop: &AtomicBool,
    report: Report<'_>,
) -> (Shown, Option<String>) {
    if target.sha.is_empty() && target.image.is_file() {
        let edit = edit::load(&target.image, &edit::dir()).ok().flatten().map(|(_, e)| e).unwrap_or_default();
        target.sha = sha(&target.image, &edit).unwrap_or_default();
    }
    let failed = act(places, &target, work, stop, report).err().filter(|_| !stop.load(Relaxed));
    let failed = failed.map(|e| format!("{e:#}"));
    let libraries = places.library(None);
    let have = |a: &wprs_depth::Asset| {
        places.installed(a).is_some() || a.kind == Part::Runtime && !libraries.is_empty()
    };
    let list: Vec<_> = (assets().iter())
        .map(|a| {
            json!({"id": a.id, "title": a.title, "size": format!("{:.1} MB", a.bytes as f64 / 1e6),
            "license": a.license, "license_url": a.license_url, "hosts": a.hosts(), "installed": have(a)})
        })
        .collect();
    let map = places.find(&target.image, &target.sha, &target.depth).map(|p| url(&p.to_string_lossy()));
    let ready = have(&MODEL) && !libraries.is_empty();
    let (assets, allowed) = (json!(list).to_string(), wprs_depth::allowed());
    (Shown { assets, map: map.unwrap_or_default(), sha: target.sha, ready, allowed }, failed)
}

fn act(places: &Places, target: &Target, work: Work, stop: &AtomicBool, report: Report<'_>) -> Result<()> {
    let (image, sha) = (&target.image, target.sha.as_str());
    match work {
        Work::Open => Ok(()),
        Work::Download if !wprs_depth::allowed() => bail!("downloads are turned off in the settings"),
        Work::Download => {
            let missing = assets().into_iter().filter(|a| places.installed(a).is_none());
            for asset in missing.filter(|a| a.kind == Part::Model || places.library(None).is_empty()) {
                let mut shown = -1.0;
                places.download(&asset, &mut |done, total| {
                    let part = (done * 200 / total.max(1)) as f64 / 200.0;
                    if part != shown {
                        shown = part;
                        report("download", part);
                    }
                    !stop.load(Relaxed)
                })?;
            }
            ready(places, target, stop, report)
        }
        Work::Generate => {
            crate::depthproc::generate(image, (target.depth.input, target.device), stop, report)
        }
        Work::Adopt(file) => places.import(&file, image, sha).map(drop),
        Work::Keep => {
            let found = places.find(image, sha, &target.depth).context("the image has no depth map yet")?;
            write(&read(&found)?, &beside(image))
        }
        Work::Install(file) => {
            let sha = sha256(&file)?;
            let asset = assets().into_iter().find(|a| a.sha256 == sha);
            places
                .install(&asset.context("this file is neither the depth model nor ONNX Runtime")?, &file)?;
            ready(places, target, stop, report)
        }
    }
}

fn ready(places: &Places, target: &Target, stop: &AtomicBool, report: Report<'_>) -> Result<()> {
    let missing = places.installed(&MODEL).is_none() || places.library(None).is_empty();
    if missing || !target.image.is_file() || places.find(&target.image, &target.sha, &target.depth).is_some()
    {
        return Ok(());
    }
    crate::depthproc::generate(&target.image, (target.depth.input, target.device), stop, report)
}

pub fn look(places: &Places, (image, sha): (&Path, &str), depth: &Depth, colour: bool) -> Result<PathBuf> {
    let found = places.find(image, sha, depth).context("the image has no depth map yet")?;
    let map = wprs_depth::shrink(wprs_depth::adjust(&wprs_depth::shrink(read(&found)?, 1024), depth), 512);
    let pixels = image::DynamicImage::ImageRgb8(wprs_depth::coloured(&map, colour)).into_rgba8();
    crate::picture::write(&pixels)
}
