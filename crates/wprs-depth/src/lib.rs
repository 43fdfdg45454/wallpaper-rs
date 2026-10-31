#[cfg(feature = "fetch")]
mod fetch;
mod files;
#[cfg(feature = "infer")]
mod infer;
#[cfg(feature = "fetch")]
mod install;
pub mod models;
mod process;
mod store;

#[cfg(feature = "fetch")]
pub use fetch::{AGENT, Cancelled, allowed, download};
pub use files::{beside, read, sha256, shape, write};
#[cfg(feature = "infer")]
pub use infer::{Bench, Device, Estimator, Stage, TELEMETRY, background, bench, decode, generate, threads};
pub use models::{Asset, MODEL, Part, RUNTIME};
pub use process::{adjust, coloured, input_size, normalize, shrink, tensor};
pub use store::CACHE_BYTES;

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
        let kind = if asset.kind == Part::Model { "models" } else { "runtime" };
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
