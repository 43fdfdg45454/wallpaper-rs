use crate::bank::Bank;
use crate::bus::Pipe;
use crate::feed::raw;
use crate::mix::{Mix, RATE, Track};
use crate::{Error, Event, lock, make, pipewire};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSrc, AppSrcCallbacks};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wprs_scene::Voice;

const CHUNK: usize = 480;
const QUEUED: Duration = Duration::from_millis(20);
const SINK_BUFFER: i64 = 40_000;
const SINK_PERIOD: i64 = 10_000;
const LATENCY: &str = "960/48000";
const QUANTUM: Duration = Duration::from_millis(20);
const SOURCE: &str = "appsrc://";

pub struct Soundtrack {
    pipe: Pipe,
    mix: Arc<Mutex<Mix>>,
    delay: Mutex<Option<Duration>>,
}

impl Soundtrack {
    pub fn open(
        bank: &Bank,
        lanes: &[(Vec<Option<usize>>, Track)],
        voices: &[Voice],
        volume: f64,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        crate::init()?;
        let mut mix = Mix::new(&lanes.iter().map(|(_, track)| track.clone()).collect::<Vec<_>>());
        mix.set(voices, 0.0);
        let mix = Arc::new(Mutex::new(mix));
        let playing = |lane: &usize| voices.get(*lane).is_some_and(|v| v.play);
        let (now, later): (Vec<usize>, Vec<usize>) = (0..lanes.len()).partition(playing);
        for lane in now.into_iter().chain(later) {
            for (clip, file) in lanes[lane].0.iter().enumerate() {
                file.iter().for_each(|&file| bank.want(file, &mix, (lane, clip)));
            }
        }
        let playbin = make("playbin")?;
        playbin.set_property("uri", SOURCE);
        playbin.set_property_from_str("flags", "audio+soft-volume");
        playbin.set_property("volume", volume.clamp(0.0, 1.0));
        playbin.set_property("mute", false);
        let piped = pipewire::sink(Some(LATENCY));
        piped.iter().for_each(|sink| playbin.set_property("audio-sink", sink));
        let feed = mix.clone();
        playbin.connect("source-setup", false, move |args| {
            let source = args[1].get::<gst::Element>().ok()?.downcast::<AppSrc>().ok()?;
            stream(&source, feed.clone());
            None
        });
        if let Some(bin) = playbin.downcast_ref::<gst::Bin>() {
            bin.connect_deep_element_added(|_, _, element| tighten(element));
        }
        let shared = crate::Shared { latency: Some(LATENCY), ..Default::default() };
        let pipe = Pipe::start(playbin, (SOURCE.into(), shared), Arc::new(sink), true)?;
        Ok(Soundtrack { pipe, mix, delay: Mutex::default() })
    }

    pub fn set(&self, voices: &[Voice]) {
        let heard = self.position().map(|d| d.saturating_sub(self.delay()).as_secs_f64() * f64::from(RATE));
        let mut mix = lock(&self.mix);
        let heard = heard.unwrap_or(mix.frames as f64);
        mix.set(voices, heard);
    }

    pub fn latency(&self) -> Option<Duration> {
        let made = lock(&self.mix).frames;
        let heard = self.position()?;
        Some(Duration::from_secs_f64(made as f64 / f64::from(RATE)).saturating_sub(heard) + self.delay())
    }

    pub fn delay(&self) -> Duration {
        let mut known = lock(&self.delay);
        if let Some(delay) = *known {
            return delay;
        }
        let playbin = &self.pipe.playbin;
        let mut query = gst::query::Latency::new();
        if playbin.current_state() != gst::State::Playing || !playbin.query(&mut query) {
            return Duration::ZERO;
        }
        let sink = playbin.property::<Option<gst::Element>>("audio-sink");
        let piped = sink.is_some_and(|s| s.name() == pipewire::SINK);
        let delay = Duration::from(query.result().1) + if piped { QUANTUM } else { Duration::ZERO };
        tracing::debug!(?delay, "scene sound output delay");
        *known.insert(delay)
    }

    pub fn quiet(&self) -> bool {
        lock(&self.mix).quiet()
    }

    pub fn set_playing(&self, playing: bool) {
        self.pipe.set_playing(playing);
    }

    pub fn set_volume(&self, volume: f64) {
        self.pipe.playbin.set_property("volume", volume.clamp(0.0, 1.0));
    }

    pub fn position(&self) -> Option<Duration> {
        self.pipe.playbin.query_position::<gst::ClockTime>().map(Duration::from)
    }

    pub fn playheads(&self) -> (u64, Vec<f64>) {
        let mix = lock(&self.mix);
        (mix.frames, mix.playheads())
    }
}

fn stream(source: &AppSrc, mix: Arc<Mutex<Mix>>) {
    source.set_caps(Some(&raw(false)));
    source.set_format(gst::Format::Time);
    source.set_max_bytes(CHUNK as u64 * 4 * 2);
    let mut at = 0u64;
    let callbacks = AppSrcCallbacks::builder().need_data(move |source, _| {
        let Ok(mut buffer) = gst::Buffer::with_size(CHUNK * 4) else { return };
        let data = buffer.get_mut().expect("a new buffer is writable");
        data.set_pts(gst::ClockTime::from_nseconds(at * 1_000_000_000 / u64::from(RATE)));
        data.set_duration(gst::ClockTime::from_nseconds(CHUNK as u64 * 1_000_000_000 / u64::from(RATE)));
        if let Ok(mut map) = data.map_writable() {
            lock(&mix).fill(&mut map);
        }
        at += CHUNK as u64;
        let _ = source.push_buffer(buffer);
    });
    source.set_callbacks(callbacks.build());
}

fn tighten(element: &gst::Element) {
    let has = |name| element.find_property(name).is_some();
    if element.factory().is_some_and(|f| f.name() == "queue") {
        element.set_property("max-size-time", QUEUED.as_nanos() as u64);
    }
    if has("buffer-time") && has("latency-time") {
        element.set_property("buffer-time", SINK_BUFFER);
        element.set_property("latency-time", SINK_PERIOD);
    }
}
