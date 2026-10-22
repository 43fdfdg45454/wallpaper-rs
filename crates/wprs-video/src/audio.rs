use crate::pipewire::{self, SINK};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::time::{Duration, Instant};

const RETRY: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
const GRACE: Duration = Duration::from_millis(500);

pub(crate) fn seek(playbin: &gst::Element, at: gst::ClockTime) {
    if let Err(e) = playbin.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, at) {
        tracing::debug!("cannot seek the video: {e}");
    }
}

struct Outputs(gst::DeviceProvider, Instant);

impl Drop for Outputs {
    fn drop(&mut self) {
        self.0.stop();
    }
}

#[derive(Default)]
pub(crate) struct Audio {
    uri: String,
    outputs: Option<Outputs>,
    delay: Duration,
    restart: Option<gst::ClockTime>,
    fresh: bool,
    hold: Option<Instant>,
}

impl Audio {
    pub(crate) fn new(uri: String) -> Self {
        Self { uri, fresh: true, ..Self::default() }
    }

    pub(crate) fn holding(&mut self) -> bool {
        self.hold = self.hold.filter(|at| Instant::now() < *at);
        self.restart.is_some() || self.fresh || self.hold.is_some()
    }

    pub(crate) fn wait(&self) -> Option<gst::ClockTime> {
        let now = Instant::now();
        let look = self.outputs.as_ref().map(|o| o.1.max(now + RETRY));
        let at = self.hold.into_iter().chain(look).min()?;
        Some(gst::ClockTime::from_mseconds(at.saturating_duration_since(now).as_millis() as u64))
    }

    pub(crate) fn tick(&mut self, playbin: &gst::Element, bus: &gst::Bus) {
        let now = Instant::now();
        let sinks = |o: &Outputs| o.1 <= now && o.0.devices().iter().any(|d| d.has_classes("Audio/Sink"));
        if self.outputs.take_if(|o| sinks(o)).is_some() {
            tracing::info!("an audio output appeared, the video plays with sound");
            self.rebuild(playbin, bus, pipewire::sink());
        }
    }

    pub(crate) fn failed(&mut self, playbin: &gst::Element, bus: &gst::Bus, err: &gst::message::Error) {
        tracing::debug!(details = err.debug().as_deref(), "PipeWire audio error");
        let reached = err.error().message().starts_with("stream error");
        if !reached && std::env::var_os("FLATPAK_ID").is_none() {
            tracing::info!("cannot reach PipeWire, using the default audio output");
            return self.rebuild(playbin, bus, None);
        }
        tracing::info!("no audio output, the video plays without sound until one appears");
        self.delay = (self.delay * 2).clamp(RETRY, RETRY_MAX);
        let provider = gst::DeviceProviderFactory::by_name("pipewiredeviceprovider");
        self.outputs =
            provider.filter(|p| p.start().is_ok()).map(|p| Outputs(p, Instant::now() + self.delay));
        let silent = gst::ElementFactory::make("fakesink").property("sync", true).build().ok();
        self.rebuild(playbin, bus, silent);
    }

    pub(crate) fn settled(&mut self, playbin: &gst::Element) {
        if let Some(at) = self.restart.take() {
            return seek(playbin, at);
        }
        let piped = playbin.property::<Option<gst::Element>>("audio-sink").is_some_and(|s| s.name() == SINK);
        if std::mem::take(&mut self.fresh) && piped {
            self.hold = Some(Instant::now() + GRACE);
        }
    }

    fn rebuild(&mut self, playbin: &gst::Element, bus: &gst::Bus, sink: Option<gst::Element>) {
        self.restart = playbin.query_position::<gst::ClockTime>().filter(|at| !at.is_zero());
        self.fresh = true;
        let _ = playbin.set_state(gst::State::Null);
        bus.set_flushing(true);
        bus.set_flushing(false);
        playbin.set_property("audio-sink", sink);
        playbin.set_property("uri", &self.uri);
        let _ = playbin.set_state(gst::State::Paused);
    }
}
