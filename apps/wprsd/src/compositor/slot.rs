use super::{Command, Reel, Settings, lock, loudness};
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, Imported, Renderer, Shared, log_allowed};
use wprs_video::{Event, Frame, Options, Player};

const RETRIES: u32 = 3;
const FIRST_RETRY: Duration = Duration::from_secs(2);
const FIRST_FRAME: Duration = Duration::from_secs(10);

pub(super) type Demand = ((u32, u32), bool, u32, u32);

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
    pub(super) fps: u32,
    pub(super) volume: u32,
    pub(super) decode: &'static str,
    failures: u32,
    inbox: Arc<Inbox>,
    shares: bool,
    imported: Option<Imported>,
    held: Option<Frame>,
    pub(super) steered: Option<(wprs_video::Cut, u32)>,
}

impl PlayerSlot {
    pub(super) fn new((max_size, playing, fps, volume): Demand, open_at: Instant) -> Self {
        let (player, texture, error, inbox, due) = (None, None, None, Arc::default(), Some(open_at));
        let (decode, shares, imported, held) =
            ("", std::env::var_os("WPRS_ZERO_COPY").is_none_or(|v| v != "0"), None, None);
        Self {
            player,
            texture,
            error,
            due,
            playing,
            max_size,
            fps,
            volume,
            decode,
            failures: 0,
            inbox,
            shares,
            imported,
            held,
            steered: None,
        }
    }

    pub(super) fn hush(&self, (settings, locked): (Settings, bool)) {
        let (volume, muted) = (
            loudness(settings.volume * settings.duck, self.volume),
            settings.muted || locked || self.volume == 0,
        );
        self.player.iter().for_each(|p| p.set_volume(volume, muted));
    }

    pub(super) fn set_volume(&mut self, volume: u32, heard: (Settings, bool), now: Instant) {
        self.volume = volume;
        let audible = volume > 0 && !heard.0.muted && heard.0.volume > 0.0;
        if self.player.as_ref().is_some_and(|p| audible && !p.audio_enabled()) {
            (self.player, self.due) = (None, Some(now));
        }
        self.hush(heard);
    }

    pub(super) fn tick(
        &mut self,
        reel: &Reel,
        heard: (Settings, bool),
        tx: &mpsc::Sender<Command>,
        now: Instant,
    ) {
        let Some(_) = self.due.take_if(|at| *at <= now) else { return };
        match self.player {
            Some(_) => {
                self.fail(&reel.path, format!("no video frame within {} s", FIRST_FRAME.as_secs()), now)
            }
            None => self.open(reel, heard, tx, now),
        }
    }

    pub(super) fn revive(&mut self, now: Instant) {
        if self.player.is_none() && self.due.is_none() {
            (self.failures, self.due) = (0, Some(now));
        }
    }

    fn open(
        &mut self,
        reel: &Reel,
        (settings, locked): (Settings, bool),
        tx: &mpsc::Sender<Command>,
        now: Instant,
    ) {
        let (max_size, max_fps, cut, path) = (self.max_size, self.fps, reel.cut, &reel.path);
        let (volume, muted) = (loudness(settings.volume, self.volume), settings.muted || self.volume == 0);
        let (inbox, tx) = (self.inbox.clone(), tx.clone());
        let options = Options { max_size, max_fps, volume, muted, cut, zero_copy: self.shares };
        let opened = Player::open(path, options, move |event| {
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
                tracing::debug!(path = %path.display(), "video opened");
                player.set_volume(volume * settings.duck, muted || locked);
                player.play_once(reel.once);
                player.set_playing(self.playing);
                (self.player, self.due, self.steered) = (Some(player), Some(now + FIRST_FRAME), None);
            }
            Err(e) => drop(lock(&self.inbox.error).replace(e.to_string())),
        }
    }

    pub(super) fn receive(&mut self, path: &Path, renderer: &Renderer, now: Instant) -> bool {
        let (frame, error) = (lock(&self.inbox.frame).take(), lock(&self.inbox.error).take());
        let uploaded = frame.is_some_and(|frame| self.upload(path, renderer, frame, now));
        match error {
            Some(error) if error == wprs_video::UNSHARED && self.shares => self.copy_instead(now),
            Some(error) => self.fail(path, error, now),
            None => {}
        }
        uploaded
    }

    fn upload(&mut self, path: &Path, renderer: &Renderer, frame: Frame, now: Instant) -> bool {
        let (data, w, h, stride) = (frame.data(), frame.width, frame.height, frame.stride);
        let uploaded = match (frame.dmabuf(), &mut self.texture) {
            (Some(shared), _) => {
                let (modifier, bt709, full, turn) = (shared.modifier, shared.bt709, shared.full, shared.turn);
                let planes = shared.planes();
                let wanted = Shared { size: (w, h), modifier, planes, bt709, full, turn, fit: self.max_size };
                let image = renderer.convert_shared(&mut self.imported, &wanted).cloned();
                image.map(|image| drop(self.texture.replace(image)))
            }
            (None, Some(texture)) if self.decode == "copy" => {
                renderer.update_rgba(texture, data, w, h, stride)
            }
            (None, _) => renderer.upload_rgba(data, w, h, stride).map(|t| drop(self.texture.replace(t))),
        };
        let shared = frame.dmabuf().is_some();
        if uploaded.is_ok() {
            self.imported = self.imported.take().filter(|_| shared);
            (self.decode, self.held) = (if shared { "zero-copy" } else { "copy" }, shared.then_some(frame));
        } else if shared {
            self.copy_instead(now);
        }
        let Err(e) = uploaded else {
            (self.error, self.failures) = (None, 0);
            self.due = self.due.filter(|_| self.player.is_none());
            return true;
        };
        static UPLOAD_ERRORS: AtomicU64 = AtomicU64::new(0);
        if log_allowed(&UPLOAD_ERRORS) {
            tracing::warn!("cannot upload a video frame: {e}");
            tracing::debug!(path = %path.display(), "video with frames that cannot be uploaded");
        }
        false
    }

    fn copy_instead(&mut self, now: Instant) {
        tracing::info!("the GPU cannot read the decoded video frames in place, copying them instead");
        (self.shares, self.player, self.imported, self.held, self.due) = (false, None, None, None, Some(now));
    }

    fn fail(&mut self, path: &Path, error: String, now: Instant) {
        self.player = None;
        self.failures += 1;
        let retry = self.failures <= RETRIES;
        self.due = retry.then(|| now + FIRST_RETRY * 2u32.pow(self.failures - 1));
        let file = std::path::absolute(path).unwrap_or_else(|_| path.into());
        let shown = error.replace(&*file.to_string_lossy(), "<video>");
        tracing::warn!(retry, "video playback failed: {shown}");
        tracing::debug!(path = %path.display(), "video that failed: {error}");
        self.error = Some(error);
    }
}

#[cfg(test)]
mod tests;
