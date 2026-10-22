use crate::{Map, Places, adjust, beside, read, shape, shrink, write};
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wprs_core::edit::Depth;

pub const CACHE_BYTES: u64 = 256 << 20;
const MAX_ASPECT: f64 = 0.02;

fn hashed(sha256: &str) -> Option<&str> {
    (sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())).then_some(sha256)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().filter(|m| m.is_file())?.modified().ok()
}

impl Places {
    pub fn generated(&self, sha256: &str, depth: &Depth) -> Option<PathBuf> {
        let name = format!("{}-{}-{}.png", hashed(sha256)?, depth.model, depth.input);
        Some(self.cache.join("depth").join(name))
    }

    pub fn own(&self, sha256: &str) -> Option<PathBuf> {
        Some(self.data.join("depth").join(format!("{}.png", hashed(sha256)?)))
    }

    pub fn find(&self, image: &Path, sha256: &str, depth: &Depth) -> Option<PathBuf> {
        let candidates = [self.own(sha256), Some(beside(image)), self.generated(sha256, depth)];
        let dated = candidates.into_iter().flatten().filter_map(|p| Some((modified(&p)?, p)));
        dated.rev().max_by_key(|(time, _)| *time).map(|(_, path)| path)
    }

    pub fn map(&self, image: &Path, sha256: &str, depth: &Depth, edge: u32) -> Result<Option<Map>> {
        let Some(path) = self.find(image, sha256, depth) else { return Ok(None) };
        Ok(Some(adjust(&shrink(read(&path)?, edge), depth)))
    }

    pub fn import(&self, file: &Path, image: &Path, sha256: &str) -> Result<PathBuf> {
        let (map, (w, h)) = (read(file)?, shape(image)?);
        let ratio = |w: u32, h: u32| f64::from(w) / f64::from(h.max(1));
        let off = ratio(map.width(), map.height()) / ratio(w, h) - 1.0;
        let (mw, mh) = map.dimensions();
        ensure!(
            off.abs() <= MAX_ASPECT,
            "the map is {mw}x{mh} and the image {w}x{h}: their shapes differ by over 2 %"
        );
        let near = beside(image);
        if !near.is_symlink() && write(&map, &near).is_ok() {
            return Ok(near);
        }
        let own = self.own(sha256).context("the image has no valid SHA-256")?;
        write(&map, &own).map(|()| own)
    }

    pub fn trim(&self, keep: &Path, limit: u64) -> Result<u64> {
        let entries = std::fs::read_dir(self.cache.join("depth"))?.flatten();
        let mut maps: Vec<(SystemTime, u64, PathBuf)> = entries
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "png") && p != keep)
            .filter_map(|p| Some((modified(&p)?, std::fs::metadata(&p).ok()?.len(), p)))
            .collect();
        maps.sort();
        let mut total =
            maps.iter().map(|m| m.1).sum::<u64>() + std::fs::metadata(keep).map_or(0, |m| m.len());
        let mut freed = 0;
        for (_, len, path) in maps {
            if total <= limit {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                (total, freed) = (total - len, freed + len);
            }
        }
        Ok(freed)
    }
}
