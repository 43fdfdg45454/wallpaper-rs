use super::{Command, Settings, lock};
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, Renderer, log_allowed};
use wprs_video::{Event, Frame, Options, Player};

const RETRIES: u32 = 3;
const FIRST_RETRY: Duration = Duration::from_secs(2);
const FIRST_FRAME: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Inbox {
    frame: Mutex<Option<Frame>>,
    error: Mutex<Option<String>>,
}

pub(super) struct PlayerSlot {
    pub(super) player: Option<Player>,
    pub(super) texture: Option<GpuImage>,
    pub(super) error: Option<String>,
    pub(super) due: Option<Instant>,
    pub(super) playing: bool,
    pub(super) max_size: (u32, u32),
    failures: u32,
    inbox: Arc<Inbox>,
}

impl PlayerSlot {
    pub(super) fn new(max_size: (u32, u32), playing: bool, open_at: Instant) -> Self {
        let (player, texture, error, inbox) = (None, None, None, Arc::default());
        Self { player, texture, error, due: Some(open_at), playing, max_size, failures: 0, inbox }
    }

    pub(super) fn tick(&mut self, path: &Path, settings: Settings, tx: &mpsc::Sender<Command>, now: Instant) {
        let Some(_) = self.due.take_if(|at| *at <= now) else { return };
        match self.player {
            Some(_) => self.fail(path, format!("no video frame within {} s", FIRST_FRAME.as_secs()), now),
            None => self.open(path, settings, tx, now),
        }
    }

    pub(super) fn revive(&mut self, now: Instant) {
        if self.player.is_none() && self.due.is_none() {
            (self.failures, self.due) = (0, Some(now));
        }
    }

    fn open(&mut self, path: &Path, settings: Settings, tx: &mpsc::Sender<Command>, now: Instant) {
        let (max_size, max_fps, volume, muted) =
            (self.max_size, settings.max_fps, settings.volume, settings.muted);
        let (inbox, tx) = (self.inbox.clone(), tx.clone());
        let opened = Player::open(path, Options { max_size, max_fps, volume, muted }, move |event| {
            let first = match event {
                Event::Frame(frame) => lock(&inbox.frame).replace(frame).is_none(),
                Event::Error(e) => lock(&inbox.error).replace(e).is_none(),
            };
            if first {
                let _ = tx.send(Command::Wake);
            }
        });
        match opened {
            Ok(player) => {
                tracing::info!(path = %path.display(), "video opened");
                player.set_playing(self.playing);
                (self.player, self.due) = (Some(player), Some(now + FIRST_FRAME));
            }
            Err(e) => drop(lock(&self.inbox.error).replace(e.to_string())),
        }
    }

    pub(super) fn receive(&mut self, path: &Path, renderer: &Renderer, now: Instant) -> bool {
        let (frame, error) = (lock(&self.inbox.frame).take(), lock(&self.inbox.error).take());
        let uploaded = frame.is_some_and(|frame| self.upload(path, renderer, &frame));
        if let Some(error) = error {
            self.fail(path, error, now);
        }
        uploaded
    }

    fn upload(&mut self, path: &Path, renderer: &Renderer, frame: &Frame) -> bool {
        let (data, w, h, stride) = (frame.data(), frame.width, frame.height, frame.stride);
        let uploaded = match &mut self.texture {
            Some(texture) => renderer.update_rgba(texture, data, w, h, stride),
            None => renderer.upload_rgba(data, w, h, stride).map(|t| drop(self.texture.replace(t))),
        };
        let Err(e) = uploaded else {
            (self.error, self.failures) = (None, 0);
            self.due = self.due.filter(|_| self.player.is_none());
            return true;
        };
        static UPLOAD_ERRORS: AtomicU64 = AtomicU64::new(0);
        if log_allowed(&UPLOAD_ERRORS) {
            tracing::warn!(path = %path.display(), "cannot upload a video frame: {e}");
        }
        false
    }

    fn fail(&mut self, path: &Path, error: String, now: Instant) {
        self.player = None;
        self.failures += 1;
        let retry = self.failures <= RETRIES;
        self.due = retry.then(|| now + FIRST_RETRY * 2u32.pow(self.failures - 1));
        tracing::warn!(path = %path.display(), retry, "video playback failed: {error}");
        self.error = Some(error);
    }
}
