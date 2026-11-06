use super::SceneState;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use wprs_render::Pixels;
use wprs_scene::{Control, Mode, ScriptOutputs, Wake};

const EVERY: Duration = Duration::from_secs(2);
const SOON: Duration = Duration::from_millis(50);
const SHOWN: usize = 8;
const VIDEOS: usize = SHOWN;

type Loaded = Vec<(String, Arc<Pixels>)>;
type Files = Vec<(String, PathBuf)>;

#[derive(Default)]
pub(in crate::compositor) struct Listed {
    pub(super) due: Option<Instant>,
    pub(super) text: String,
    files: Files,
    pub(super) pixels: Loaded,
    asked: Vec<String>,
    pub(super) reels: Vec<(String, super::super::Reel)>,
    loading: Option<mpsc::Receiver<Loaded>>,
    scanning: Option<mpsc::Receiver<(String, Files)>>,
    pub(super) unset: bool,
    watch: Option<watch::Watch>,
    wake: Option<watch::Wake>,
}

mod scan;
mod watch;

use scan::scan;

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

    pub(in crate::compositor) fn waker(&mut self, wake: impl FnOnce() -> watch::Wake) {
        self.listed.wake.get_or_insert_with(wake);
    }

    pub(in crate::compositor) fn heard_folder(&mut self, now: Instant) {
        let told = self.listed.watch.as_ref().is_some_and(|w| w.changed.load(Ordering::Acquire));
        if self.lists() && told && self.due.is_none_or(|at| at > now) {
            self.due = Some(now);
        }
    }

    pub(super) fn listing_wake(&self, now: Instant) -> Wake {
        let told = self.listed.watch.as_ref().map(|w| w.changed.load(Ordering::Acquire));
        match (self.lists(), self.listed.scanning.is_some(), told) {
            (false, ..) => Wake::Idle,
            _ if self.listed.unset => Wake::Idle,
            (_, true, _) if self.listed.wake.is_some() => Wake::Idle,
            (_, true, _) => Wake::At(SOON.as_secs_f64()),
            (_, _, Some(true)) => Wake::At(0.0),
            (_, _, Some(false)) => Wake::Idle,
            (_, _, None) => {
                Wake::At(self.listed.due.map_or(0.0, |at| at.saturating_duration_since(now).as_secs_f64()))
            }
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
        let chosen = |id: String| {
            let dir = self.spec.values.get(&id).and_then(|v| v.as_str()).filter(|v| v.starts_with('/'));
            let prop = self.spec.pack.props.properties.get(&id).cloned();
            let dir = dir.map(PathBuf::from).zip(prop);
            (id, dir)
        };
        let wanted: Vec<_> = self.folders().into_iter().map(chosen).collect();
        let deep = |p: &wprs_scene::Property| match p.control {
            wprs_scene::Control::Folder { recursive, .. } => recursive,
            _ => 0,
        };
        let dirs: Vec<PathBuf> = wanted
            .iter()
            .filter_map(|(_, d)| d.as_ref())
            .flat_map(|(d, p)| watch::dirs(d, deep(p)))
            .collect();
        if self.listed.watch.as_ref().map(|w| &w.dirs) != Some(&dirs) {
            let wake = self.listed.wake.clone().filter(|_| !dirs.is_empty());
            self.listed.watch = wake.and_then(|wake| watch::Watch::new(dirs, wake));
        }
        let told = self.listed.watch.as_ref().is_some_and(|w| w.changed.swap(false, Ordering::AcqRel));
        let polled = self.listed.watch.is_none() && self.listed.due.is_none_or(|at| now >= at);
        if self.listed.scanning.is_none() && (told || polled || self.listed.due.is_none()) {
            self.listed.due = Some(now + EVERY);
            let (tx, rx) = mpsc::channel();
            self.listed.unset = wanted.iter().all(|(_, dir)| dir.is_none());
            let wake = self.listed.wake.clone();
            let sent = move || {
                drop(tx.send(scan(wanted)));
                wake.iter().for_each(|w| w());
            };
            match self.listed.unset {
                true => sent(),
                false => drop(std::thread::spawn(sent)),
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
        let (videos, paths): (Vec<_>, Vec<_>) = paths.into_iter().partition(|(_, p)| wprs_video::is_video(p));
        let reels: Vec<_> =
            videos.into_iter().take(VIDEOS).map(|(k, p)| (k, super::super::Reel::plain(p))).collect();
        if reels != self.listed.reels {
            self.listed.reels = reels;
            let spec = self.spec.clone();
            self.update(spec);
        }
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
