use crate::Places;
use crate::files::{folder, read_png, sweep};
use anyhow::{Context, Result, ensure};
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use wprs_core::Fill;
use wprs_core::edit::Depth;

pub const SUFFIX: &str = ".layers.png";
const FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub fill: Fill,
    pub count: usize,
    pub shift: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub layer: usize,
    pub reach: f32,
    pub pixels: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Info {
    pub format: u32,
    pub size: [u32; 2],
    pub fill: String,
    pub cuts: Vec<f32>,
    pub depths: Vec<f32>,
    pub bands: Vec<Band>,
}

#[derive(Clone)]
pub struct Layers {
    pub info: Info,
    pub images: Vec<RgbaImage>,
}

fn json(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().replace(SUFFIX, ".layers.json"));
    path.with_file_name(name.unwrap_or_default())
}

pub(crate) fn unjson(png: &Path) {
    if png.to_string_lossy().ends_with(SUFFIX) {
        std::fs::remove_file(json(png)).ok();
    }
}

impl Settings {
    pub fn named(&self) -> String {
        format!("{}-{}-{:.0}", self.fill.as_str(), self.count, self.shift * 1000.0)
    }
}

impl Places {
    pub fn layer_set(&self, image: &Path, sha256: &str, depth: &Depth, set: &Settings) -> Option<PathBuf> {
        let map = self.find(image, sha256, depth)?;
        let meta = std::fs::metadata(&map).ok()?;
        let stamp = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
        let (near, far, gamma) = (depth.near, depth.far, depth.gamma);
        let used = format!(
            "{FORMAT}|{sha256}|{}|{}|{near:.3}|{far:.3}|{gamma:.3}|{}|{}|{stamp}|{}|{}|{}",
            depth.model,
            depth.input,
            set.named(),
            meta.len(),
            crate::models::LAMA.id,
            crate::build::VERSION,
            serde_json::to_string(&(&depth.cuts, &depth.strokes, &depth.fills)).unwrap_or_default()
        );
        let hash: String =
            Sha256::digest(used.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
        let name = format!("{}-{hash}{SUFFIX}", crate::files::hashed(sha256)?);
        Some(self.cache.join("depth").join(name))
    }

    pub fn fill(&self, wanted: Fill) -> Fill {
        let ready = self.installed(&crate::models::LAMA).is_some() && !self.library(None).is_empty();
        if wanted == Fill::Lama && !ready { Fill::Simple } else { wanted }
    }

    pub fn discard(&self, path: &Path) {
        std::fs::remove_file(path).ok();
        unjson(path);
    }

    pub fn layers_of(&self, sha256: &str) -> Vec<PathBuf> {
        let prefix = format!("{sha256}-");
        let entries = std::fs::read_dir(self.cache.join("depth")).into_iter().flatten().flatten();
        let named = entries.filter(|e| e.file_name().to_string_lossy().starts_with(&prefix));
        named.map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(SUFFIX)).collect()
    }

    pub fn unlayer(&self, sha256: &str) -> usize {
        let sets = self.layers_of(sha256);
        let gone = |p: &PathBuf| std::fs::remove_file(json(p)).is_ok() | std::fs::remove_file(p).is_ok();
        sets.iter().filter(|p| gone(p)).count()
    }
}

pub fn save(path: &Path, layers: &Layers) -> Result<()> {
    let Layers { info, images } = layers;
    let (w, h) = (info.size[0], info.size[1]);
    ensure!(images.iter().all(|i| i.dimensions() == (w, h)), "the layers differ in size");
    let dir = path.parent().context("no folder for the layers")?;
    folder(dir)?;
    sweep(dir, &[".layers-"]);
    let publish = |name: &Path, write: &mut dyn FnMut(&mut std::fs::File) -> Result<()>| -> Result<()> {
        let mut file = tempfile::Builder::new().prefix(".layers-").tempfile_in(dir)?;
        write(file.as_file_mut())?;
        file.as_file().sync_all()?;
        file.persist(name).map(drop).map_err(|e| e.error.into())
    };
    publish(&json(path), &mut |f| Ok(f.write_all(&serde_json::to_vec(info)?)?))?;
    let atlas: Vec<u8> = images.iter().flat_map(|i| i.as_raw().iter().copied()).collect();
    let rows = h * images.len() as u32;
    publish(path, &mut |f| {
        let encoder = PngEncoder::new(std::io::BufWriter::new(&mut *f));
        Ok(encoder.write_image(&atlas, w, rows, ExtendedColorType::Rgba8)?)
    })
}

pub fn load(path: &Path) -> Result<Option<Layers>> {
    let Ok(text) = std::fs::read(json(path)) else { return Ok(None) };
    let info: Info = serde_json::from_slice(&text)?;
    ensure!(info.format == FORMAT && !info.depths.is_empty(), "the layer set is not understood");
    let atlas = read_png(path)?.into_rgba8();
    let (w, h) = (info.size[0], info.size[1]);
    ensure!(atlas.width() == w && atlas.height() == h * info.depths.len() as u32, "the layer set is torn");
    let cut = |k: u32| image::imageops::crop_imm(&atlas, 0, k * h, w, h).to_image();
    Ok(Some(Layers { images: (0..info.depths.len() as u32).map(cut).collect(), info }))
}
