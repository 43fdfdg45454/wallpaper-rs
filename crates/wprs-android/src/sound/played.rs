use super::{LINGER, RESYNC, locked, out::Out};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_mix::Mix;
use wprs_scene::Voice;

pub(super) struct Played {
    pub(super) out: Out,
    pub(super) mix: Arc<Mutex<Mix>>,
    pub(super) fade: Duration,
    pub(super) playing: bool,
    pub(super) voices: Vec<Voice>,
    pub(super) synced: Instant,
    pub(super) quiet: Option<Instant>,
}

impl Played {
    pub(super) fn retime(&mut self, voices: &[Voice], now: Instant) {
        let mut mix = locked(&self.mix);
        let heard = mix.frames.saturating_sub(self.out.latency()) as f64;
        mix.set(voices, heard);
        self.voices.clear();
        self.voices.extend_from_slice(voices);
        self.synced = now;
    }

    pub(super) fn hush(&mut self, silent: bool, now: Instant) -> bool {
        self.quiet = silent.then(|| self.quiet.unwrap_or(now));
        self.quiet.is_some_and(|at| at + self.fade <= now)
    }

    pub(super) fn timers(&self, now: Instant) -> impl Iterator<Item = Duration> {
        let resync = self.playing.then(|| self.synced + RESYNC);
        let linger = if self.playing { Duration::ZERO } else { LINGER };
        let quiet = self.quiet.map(|at| at + self.fade + linger);
        resync.into_iter().chain(quiet).map(move |at| at.saturating_duration_since(now))
    }
}
