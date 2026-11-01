use super::SceneState;
use std::hash::{BuildHasher, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, mpsc};
use std::time::{Duration, Instant};
use wprs_render::Pixels;
use wprs_scene::{Control, Mode, ScriptOutputs, Wake};

const EVERY: Duration = Duration::from_secs(2);
const SOON: Duration = Duration::from_millis(50);
const SHOWN: usize = 8;
static SALT: LazyLock<std::hash::RandomState> = LazyLock::new(std::hash::RandomState::new);

type Loaded = Vec<(String, Arc<Pixels>)>;
type Files = Vec<(String, PathBuf)>;
type Wanted = Vec<(String, Option<(PathBuf, wprs_scene::Property)>)>;

#[derive(Default)]
pub(in crate::compositor) struct Listed {
    pub(super) due: Option<Instant>,
    pub(super) text: String,
    files: Files,
    pub(super) pixels: Loaded,
    asked: Vec<String>,
    loading: Option<mpsc::Receiver<Loaded>>,
    scanning: Option<mpsc::Receiver<(String, Files)>>,
    pub(super) unset: bool,
}

fn opaque(parts: &[&[u8]]) -> String {
    let mut h = SALT.build_hasher();
    parts.iter().for_each(|p| h.write(p));
    format!("{:016x}", h.finish())
}

fn scan(wanted: Wanted) -> (String, Files) {
    let (mut listing, mut files) = (serde_json::Map::new(), Vec::new());
    for (id, chosen) in wanted {
        let found = chosen.map(|(dir, prop)| crate::picks::files(&dir, &prop)).unwrap_or_default();
        let entries = found.into_iter().filter(|p| !wprs_video::is_video(p)).map(|path| {
            let stamp = std::fs::metadata(&path).map_or([0; 2], |m| [m.size() as i64, m.mtime()]);
            let raw = path.as_os_str().as_encoded_bytes();
            let token = opaque(&[id.as_bytes(), raw, &stamp[0].to_le_bytes(), &stamp[1].to_le_bytes()]);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let entry = serde_json::json!([token, opaque(&[id.as_bytes(), raw]), name]);
            files.push((format!("wpak-user://{id}/{token}"), path));
            entry
        });
        let entries = serde_json::Value::Array(entries.collect());
        listing.insert(id, entries);
    }
    (serde_json::Value::Object(listing).to_string(), files)
}

impl SceneState {
    pub(in crate::compositor) fn folders(&self) -> Vec<String> {
        let all = |c: &Control| matches!(c, Control::Folder { mode: Mode::All, .. });
        self.spec
            .pack
            .props
            .properties
            .iter()
            .filter(|(_, p)| all(&p.control))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn lists(&self) -> bool {
        !self.still && self.runtime.scene.scripts.iter().any(|s| s.wants(wprs_scene::ScriptApi::Folders))
    }

    pub(super) fn listing_wake(&self, now: Instant) -> Wake {
        match (self.lists(), self.listed.scanning.is_some(), self.listed.due) {
            (false, ..) => Wake::Idle,
            _ if self.listed.unset => Wake::Idle,
            (_, true, _) => Wake::At(SOON.as_secs_f64()),
            (_, _, due) => Wake::At(due.map_or(0.0, |at| at.saturating_duration_since(now).as_secs_f64())),
        }
    }

    pub(super) fn listing(&mut self, now: Instant) -> bool {
        if !self.lists() {
            return false;
        }
        let found = self.listed.scanning.as_ref().map(mpsc::Receiver::try_recv);
        let mut changed = false;
        match found {
            Some(Ok((text, files))) => {
                self.listed.scanning = None;
                if text != self.listed.text {
                    (self.listed.text, self.listed.files, changed) = (text, files, true);
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => self.listed.scanning = None,
            _ => {}
        }
        if self.listed.scanning.is_none() && self.listed.due.is_none_or(|at| now >= at) {
            self.listed.due = Some(now + EVERY);
            let chosen = |id: String| {
                let dir = self.spec.values.get(&id).and_then(|v| v.as_str()).filter(|v| v.starts_with('/'));
                let prop = self.spec.pack.props.properties.get(&id).cloned();
                let dir = dir.map(PathBuf::from).zip(prop);
                (id, dir)
            };
            let wanted: Vec<_> = self.folders().into_iter().map(chosen).collect();
            let (tx, rx) = mpsc::channel();
            self.listed.unset = wanted.iter().all(|(_, dir)| dir.is_none());
            match self.listed.unset {
                true => drop(tx.send(scan(wanted))),
                false => drop(std::thread::spawn(move || drop(tx.send(scan(wanted))))),
            }
            self.listed.scanning = Some(rx);
            return self.listing(now) || changed;
        }
        changed
    }

    pub(super) fn show(&mut self, outputs: &ScriptOutputs) {
        let mut asked: Vec<String> = outputs.layers.values().filter_map(|l| l.file.clone()).collect();
        asked.sort();
        asked.dedup();
        asked.truncate(SHOWN);
        if asked == self.listed.asked {
            return;
        }
        self.listed.asked = asked.clone();
        let paths: Vec<(String, PathBuf)> = asked
            .into_iter()
            .filter_map(|key| {
                let found = self.listed.files.iter().find(|(k, _)| *k == key)?;
                Some(found.clone())
            })
            .collect();
        let kept: Loaded =
            self.listed.pixels.iter().filter(|(k, _)| paths.iter().any(|(p, _)| p == k)).cloned().collect();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let fresh = paths.into_iter().filter(|(k, _)| !kept.iter().any(|(old, _)| old == k));
            let decoded = fresh.filter_map(|(key, path)| {
                Some((key, Pixels::new(wprs_render::decode_image(&path, 4096).ok()?)))
            });
            drop(tx.send(kept.iter().cloned().chain(decoded).collect()));
        });
        self.listed.loading = Some(rx);
    }

    pub(in crate::compositor) fn loaded(&mut self) -> bool {
        let Some(pixels) = self.listed.loading.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return false;
        };
        (self.listed.pixels, self.listed.loading) = (pixels, None);
        let spec = self.spec.clone();
        self.update(spec)
    }
}

#[cfg(test)]
mod tests;
