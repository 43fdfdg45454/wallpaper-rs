use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use wprs_mix::Mix;

use super::bank::{FAILED, LOADED};
use super::feed::BLOCKS;

#[cfg(target_os = "android")]
use ndk::audio::{
    AudioCallbackResult, AudioContentType, AudioDirection, AudioFormat, AudioPerformanceMode,
    AudioSharingMode, AudioStream, AudioStreamBuilder, AudioUsage, SessionId,
};

const CALLBACK: i32 = 2400;
const SECOND: i64 = wprs_mix::RATE as i64;

static OPENED: AtomicU32 = AtomicU32::new(0);
static RELEASED: AtomicU32 = AtomicU32::new(0);
static SESSION: AtomicI32 = AtomicI32::new(-1);
static PLAYING: AtomicBool = AtomicBool::new(false);

pub(crate) fn report() -> Value {
    json!({ "opened": OPENED.load(Relaxed), "released": RELEASED.load(Relaxed), "session": SESSION.load(Relaxed),
        "playing": PLAYING.load(Relaxed), "decoded": LOADED.load(Relaxed), "undecodable": FAILED.load(Relaxed),
        "streamed": BLOCKS.load(Relaxed) })
}

struct Core {
    mix: Arc<Mutex<Mix>>,
    level: AtomicU32,
}

pub(super) struct Out {
    core: Arc<Core>,
    #[cfg(target_os = "android")]
    stream: AudioStream,
}

#[cfg(target_os = "android")]
unsafe impl Send for Out {}

#[cfg(target_os = "android")]
fn session(stream: &AudioStream) -> i32 {
    match stream.session_id() {
        SessionId::Allocated(id) => id.get(),
        SessionId::None => 0,
    }
}

impl Out {
    #[cfg(target_os = "android")]
    pub(super) fn open(mix: &Arc<Mutex<Mix>>) -> Option<Out> {
        let core = Arc::new(Core { mix: mix.clone(), level: AtomicU32::new(0) });
        let (heard, mut gain) = (core.clone(), 0.0f32);
        let callback = Box::new(move |_: &AudioStream, data: *mut std::ffi::c_void, frames: i32| {
            let out =
                unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), frames.max(0) as usize * 4) };
            match heard.mix.try_lock() {
                Ok(mut mix) => mix.fill(out),
                Err(_) => out.fill(0),
            }
            let target = f32::from_bits(heard.level.load(Relaxed)) * crate::page::ducking();
            super::scale(out, std::mem::replace(&mut gain, target), target);
            AudioCallbackResult::Continue
        });
        let stream = AudioStreamBuilder::new()
            .inspect_err(|e| log::warn!("cannot open the audio output: {e}"))
            .ok()?
            .direction(AudioDirection::Output)
            .format(AudioFormat::PCM_I16)
            .channel_count(2)
            .sample_rate(wprs_mix::RATE as i32)
            .usage(AudioUsage::Media)
            .content_type(AudioContentType::Movie)
            .performance_mode(AudioPerformanceMode::PowerSaving)
            .sharing_mode(AudioSharingMode::Shared)
            .session_id(None)
            .frames_per_data_callback(CALLBACK)
            .data_callback(callback)
            .open_stream()
            .inspect_err(|e| log::warn!("cannot open the audio output: {e}"))
            .ok()?;
        OPENED.fetch_add(1, Relaxed);
        SESSION.store(session(&stream), Relaxed);
        Some(Out { core, stream })
    }

    #[cfg(not(target_os = "android"))]
    pub(super) fn open(mix: &Arc<Mutex<Mix>>) -> Option<Out> {
        OPENED.fetch_add(1, Relaxed);
        SESSION.store(1, Relaxed);
        Some(Out { core: Arc::new(Core { mix: mix.clone(), level: AtomicU32::new(0) }) })
    }

    pub(super) fn level(&self, level: f32) {
        self.core.level.store(level.to_bits(), Relaxed);
    }

    pub(super) fn play(&self, on: bool) {
        PLAYING.store(on, Relaxed);
        #[cfg(target_os = "android")]
        let _ = if on {
            self.stream.request_start()
        } else {
            self.stream.request_pause().and_then(|()| self.stream.request_flush())
        };
    }

    pub(super) fn latency(&self) -> u64 {
        #[cfg(target_os = "android")]
        return (self.stream.frames_written() - self.stream.frames_read()).clamp(0, SECOND / 2) as u64;
        #[cfg(not(target_os = "android"))]
        return SECOND as u64 / 20;
    }
}

impl Drop for Out {
    fn drop(&mut self) {
        RELEASED.fetch_add(1, Relaxed);
        SESSION.store(-1, Relaxed);
        PLAYING.store(false, Relaxed);
    }
}
