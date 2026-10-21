use super::state::State;
use super::{Command, Settings, lock};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use wprs_render::GpuImage;
use wprs_video::{Event, Frame, Options, Player};

pub(super) struct PlayerSlot {
    pub(super) player: Option<Player>,
    pub(super) texture: Option<GpuImage>,
    pub(super) error: Arc<Mutex<Option<String>>>,
    pub(super) playing: bool,
    latest: Arc<Mutex<Option<Frame>>>,
}

impl PlayerSlot {
    fn open(path: &Path, max_size: (u32, u32), settings: Settings, tx: mpsc::Sender<Command>) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let options =
            Options { max_size, max_fps: settings.max_fps, volume: settings.volume, muted: settings.muted };
        let (frames, errors, tx, file) =
            (latest.clone(), error.clone(), Mutex::new(tx), path.display().to_string());
        let player = Player::open(path, options, move |event| {
            let wake = match event {
                Event::Frame(frame) => lock(&frames).replace(frame).is_none(),
                Event::Error(e) => {
                    tracing::warn!(path = file, "video playback failed: {e}");
                    lock(&errors).replace(e);
                    true
                }
            };
            if wake {
                let _ = lock(&tx).send(Command::Wake);
            }
        })
        .inspect(|_| tracing::info!(path = %path.display(), "video opened"))
        .inspect_err(|e| {
            tracing::warn!(path = %path.display(), "cannot play the video: {e}");
            lock(&error).replace(e.to_string());
        })
        .ok();
        Self { player, texture: None, error, playing: true, latest }
    }
}

impl State {
    pub(super) fn sync_players(&mut self) {
        let mut wanted: HashMap<PathBuf, ((u32, u32), bool)> = HashMap::new();
        for output in self.outputs.values() {
            for path in output.layers().filter_map(|s| s.content.video()) {
                let ((w, h), active) = wanted.entry(path.clone()).or_default();
                (*w, *h) = ((*w).max(output.size.0), (*h).max(output.size.1));
                *active |= output.active;
            }
        }
        self.players.retain(|path, _| wanted.contains_key(path));
        for (path, (max_size, active)) in wanted {
            let slot = match self.players.entry(path) {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(e) => {
                    let slot = PlayerSlot::open(e.key(), max_size, self.settings, self.tx.clone());
                    e.insert(slot)
                }
            };
            if slot.playing != active {
                slot.player.iter().for_each(|p| p.set_playing(active));
                slot.playing = active;
            }
        }
    }

    pub(super) fn take_video_frames(&mut self) {
        for (path, slot) in &mut self.players {
            let Some(frame) = lock(&slot.latest).take() else { continue };
            let (data, w, h, stride) = (&frame.data, frame.width, frame.height, frame.stride);
            let uploaded = match &mut slot.texture {
                Some(texture) => self.renderer.update_rgba(texture, data, w, h, stride),
                None => self.renderer.upload_rgba(data, w, h, stride).map(|t| drop(slot.texture.replace(t))),
            };
            if let Err(e) = uploaded {
                tracing::warn!(path = %path.display(), "cannot upload a video frame: {e}");
                continue;
            }
            self.outputs.values_mut().filter(|o| o.uses_video(path)).for_each(|o| o.dirty = true);
        }
    }
}
