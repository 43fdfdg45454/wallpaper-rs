mod brush;
mod build;
#[cfg(feature = "fetch")]
mod fetch;
mod files;
mod fill;
mod guided;
#[cfg(feature = "infer")]
mod infer;
#[cfg(feature = "infer")]
mod inpaint;
#[cfg(feature = "fetch")]
mod install;
#[cfg(feature = "infer")]
mod made;
pub mod models;
mod normals;
mod plan;
mod process;
#[cfg(feature = "infer")]
mod rebuild;
mod split;
mod stack;
mod store;
mod views;
mod windows;

pub use brush::{lift, lift_map, marks, stamp};
pub use build::{EDGE, MARGIN, band, layers, reach};
#[cfg(feature = "fetch")]
pub use fetch::{AGENT, Cancelled, allowed, download};
pub use files::{beside, decode, read, sha256, shape, write};
pub use fill::{push_pull, simple};
pub use guided::{EPSILON, RADIUS, filter, planes, refine};
#[cfg(feature = "infer")]
pub use infer::{Device, Estimator, Stage, Stopped, TELEMETRY, background, threads};
#[cfg(feature = "infer")]
pub use inpaint::Inpainter;
#[cfg(feature = "infer")]
pub use made::{Bench, bench, generate, generate_until};
pub use models::{Asset, LAMA, MODEL, Part, RUNTIME};
pub use normals::{MAX_BLUR, MAX_STRENGTH, normals, relief, sphere};
pub use process::{adjust, coloured, input_size, normalize, shrink, tensor};
#[cfg(feature = "infer")]
pub use rebuild::settings;
pub use split::{Split, cut, distance, indices, mask, soft, split};
pub use stack::{Band, Info, Layers, SUFFIX, Settings, load, save};
pub use store::CACHE_BYTES;
pub use views::{band_mask, halves, histogram, hue, outlines, tint};
pub use windows::{Inpaint, lama, plan};

use std::path::{Path, PathBuf};

pub type Map = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Places {
    pub data: PathBuf,
    pub cache: PathBuf,
}

impl Default for Places {
    fn default() -> Self {
        let root = |dir: Option<PathBuf>| dir.unwrap_or_else(|| PathBuf::from(".")).join("wallpaper-rs");
        Self { data: root(dirs::data_dir()), cache: root(dirs::cache_dir()) }
    }
}

impl Places {
    pub fn installed(&self, asset: &Asset) -> Option<PathBuf> {
        Some(self.file(asset)).filter(|p| p.is_file())
    }

    pub fn file(&self, asset: &Asset) -> PathBuf {
        let kind = if asset.kind == Part::Runtime { "runtime" } else { "models" };
        self.data.join(kind).join(asset.id).join(asset.file)
    }

    pub fn library(&self, explicit: Option<&Path>) -> Vec<PathBuf> {
        let env = std::env::var_os("ORT_DYLIB_PATH").filter(|v| !v.is_empty()).map(PathBuf::from);
        let dirs = ["/app/lib", "/usr/lib64", "/usr/lib", "/usr/local/lib"].map(PathBuf::from);
        let arch = PathBuf::from(format!("/usr/lib/{}-linux-gnu", std::env::consts::ARCH));
        let names = ["libonnxruntime.so", "libonnxruntime.so.1"];
        let system = dirs.into_iter().chain([arch]).flat_map(|d| names.map(|n| d.join(n)));
        let downloaded = RUNTIME.and_then(|r| self.installed(&r));
        let found = explicit.map(Path::to_path_buf).into_iter().chain(env).chain(system).chain(downloaded);
        found.filter(|p| p.is_file()).collect()
    }
}
