use super::SceneState;
use std::hash::{BuildHasher, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, mpsc};
use std::time::{Duration, Instant};
use wprs_render::Pixels;
use wprs_scene::{Control, Mode, ScriptOutputs};

const EVERY: Duration = Duration::from_secs(2);
const SHOWN: usize = 8;
static SALT: LazyLock<std::hash::RandomState> = LazyLock::new(std::hash::RandomState::new);

type Loaded = Vec<(String, Arc<Pixels>)>;

#[derive(Default)]
pub(in crate::compositor) struct Listed {
    due: Option<Instant>,
    pub(super) text: String,
    files: Vec<(String, PathBuf)>,
    pub(super) pixels: Loaded,
    asked: Vec<String>,
    loading: Option<mpsc::Receiver<Loaded>>,
}

fn opaque(parts: &[&[u8]]) -> String {
    let mut h = SALT.build_hasher();
    parts.iter().for_each(|p| h.write(p));
    format!("{:016x}", h.finish())
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

    pub(super) fn listing(&mut self, now: Instant) {
        if self.listed.due.is_some_and(|at| now < at)
            || !self.runtime.scene.scripts.iter().any(|s| s.wants(wprs_scene::ScriptApi::Folders))
        {
            return;
        }
        self.listed.due = Some(now + EVERY);
        let (mut listing, mut files) = (serde_json::Map::new(), Vec::new());
        for id in self.folders() {
            let dir = self.spec.values.get(&id).and_then(|v| v.as_str()).filter(|v| v.starts_with('/'));
            let found = dir
                .zip(self.spec.pack.props.properties.get(&id))
                .map(|(d, p)| crate::picks::files(d.as_ref(), p));
            let entries: Vec<serde_json::Value> = (found.unwrap_or_default().into_iter())
                .filter(|p| !wprs_video::is_video(p))
                .map(|path| {
                    let meta = std::fs::metadata(&path).ok();
                    let stamp = meta.map_or([0; 2], |m| [m.size() as i64, m.mtime()]);
                    let raw = path.as_os_str().as_encoded_bytes();
                    let token =
                        opaque(&[id.as_bytes(), raw, &stamp[0].to_le_bytes(), &stamp[1].to_le_bytes()]);
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let entry = serde_json::json!([token, opaque(&[id.as_bytes(), raw]), name]);
                    files.push((format!("wpak-user://{id}/{token}"), path));
                    entry
                })
                .collect();
            listing.insert(id, serde_json::Value::Array(entries));
        }
        let text = serde_json::Value::Object(listing).to_string();
        if text != self.listed.text {
            (self.listed.text, self.listed.files) = (text, files);
        }
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
