use crate::pack::Pack;
use bank::Bank;
use out::Out;
use played::Played;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_core::settings::Playback;
use wprs_mix::{Mix, Track};
use wprs_scene::{Runtime, VOLUME, Voice, Wake};

mod bank;
mod decode;
mod feed;
mod out;
mod played;

#[cfg(any(test, target_os = "android"))]
pub(crate) use out::report;

const LINGER: Duration = Duration::from_secs(5);
const RESYNC: Duration = Duration::from_secs(1);
const RETRY: Duration = Duration::from_secs(10);
const FRAME: Duration = Duration::from_millis(33);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Want {
    Off,
    Held,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Gate {
    pub(crate) silent: bool,
    pub(crate) shown: bool,
    pub(crate) running: bool,
}

pub(crate) fn decide(playback: &Playback, prop: f64, gate: Gate) -> (Want, f32) {
    let level = (playback.volume.clamp(0.0, 1.0) * prop.clamp(0.0, 100.0) / 100.0) as f32;
    let audible = !gate.silent && !playback.muted && playback.scene_sound && level > 0.0;
    let want = match () {
        _ if !audible => Want::Off,
        _ if gate.shown && gate.running => Want::On,
        _ => Want::Held,
    };
    (want, level)
}

pub(crate) fn scale(out: &mut [u8], from: f32, to: f32) {
    if from == 1.0 && to == 1.0 {
        return;
    }
    let step = (to - from) / (out.len() / 4).max(1) as f32;
    for (i, frame) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let gain = from + step * i as f32;
        for pair in frame.as_chunks_mut::<2>().0 {
            *pair = ((f32::from(i16::from_le_bytes(*pair)) * gain) as i16).to_le_bytes();
        }
    }
}

pub(crate) fn property(runtime: &Runtime) -> f64 {
    runtime.props().get(VOLUME).and_then(serde_json::Value::as_f64).unwrap_or(100.0)
}

pub(crate) struct Sounds {
    pack: Arc<Pack>,
    bank: Bank,
    played: Option<Played>,
    failed: Option<Instant>,
    voices: Vec<Voice>,
}

fn after(wake: Wake) -> Option<Duration> {
    match wake {
        Wake::Frame => Some(FRAME),
        Wake::At(seconds) => Some(Duration::from_secs_f64(seconds.clamp(0.0, 86_400.0)).max(FRAME)),
        Wake::Idle => None,
    }
}

fn locked(mix: &Mutex<Mix>) -> MutexGuard<'_, Mix> {
    mix.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Sounds {
    pub(crate) fn new(pack: Arc<Pack>) -> Sounds {
        Sounds { bank: Bank::new(&pack), pack, played: None, failed: None, voices: Vec::new() }
    }

    pub(crate) fn sounding(&self) -> bool {
        self.played.as_ref().is_some_and(|p| p.playing)
    }

    fn start(&mut self, runtime: &Runtime, voices: &[Voice], now: Instant) {
        if self.failed.is_some_and(|at| at + RETRY > now) {
            return;
        }
        let sounds = self.pack.scene.sounds();
        let lists = runtime.playlists().iter().cloned();
        let tracks: Vec<_> = lists
            .zip(sounds)
            .map(|(list, s)| Track { list, fade_in: s.fade_in, fade_out: s.fade_out })
            .collect();
        let fade = tracks.iter().map(|t| Duration::from_secs_f32(t.fade_out)).max().unwrap_or_default();
        let mut mix = Mix::new(&tracks);
        mix.set(voices, 0.0);
        let mix = Arc::new(Mutex::new(mix));
        self.failed = None;
        match Out::open(&mix) {
            Some(out) => {
                self.bank.fill(&self.pack, &mix, voices);
                let (voices, quiet) = (voices.to_vec(), None);
                self.played = Some(Played { out, mix, fade, playing: false, voices, synced: now, quiet });
            }
            None => self.failed = Some(now),
        }
    }

    pub(crate) fn follow(
        &mut self,
        runtime: &Runtime,
        (want, level): (Want, f32),
        now: Instant,
    ) -> Option<Duration> {
        if self.pack.sounds.is_empty() {
            return None;
        }
        if want != Want::On {
            self.played = None;
            if want == Want::Off {
                self.bank.clear();
            }
            return None;
        }
        let mut voices = std::mem::take(&mut self.voices);
        let wake = runtime.voices_into(&mut voices);
        let next = self.followed(runtime, &voices, (wake, level), now);
        self.voices = voices;
        next
    }

    fn followed(
        &mut self,
        runtime: &Runtime,
        voices: &[Voice],
        at: (Wake, f32),
        now: Instant,
    ) -> Option<Duration> {
        let (wake, level) = at;
        let heard = voices.iter().any(|v| v.play && v.volume > 0.0);
        if self.played.is_none() && heard {
            self.start(runtime, voices, now);
        }
        let retry = self.failed.map(|at| (at + RETRY).saturating_duration_since(now));
        let Some(played) = self.played.as_mut() else { return after(wake).into_iter().chain(retry).min() };
        let ended = locked(&played.mix).quiet();
        let faded = played.hush(!heard || ended, now);
        if played.voices != voices || (played.playing && now >= played.synced + RESYNC) {
            played.retime(voices, now);
        }
        played.out.level(level);
        if played.playing == faded {
            played.playing = !faded;
            played.out.play(played.playing);
        }
        if played.quiet.is_some_and(|at| at + played.fade + LINGER <= now) {
            self.played = None;
            return after(wake);
        }
        after(wake).into_iter().chain(played.timers(now)).min()
    }

    #[cfg(test)]
    pub(crate) fn heard(&self, frames: usize) -> Vec<[i16; 2]> {
        let Some(played) = self.played.as_ref() else { return Vec::new() };
        let mut bytes = vec![0u8; frames * 4];
        locked(&played.mix).fill(&mut bytes);
        let pairs = bytes.as_chunks::<4>().0.iter();
        pairs.map(|f| [i16::from_le_bytes([f[0], f[1]]), i16::from_le_bytes([f[2], f[3]])]).collect()
    }
}
