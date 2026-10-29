use crate::pipewire::{self, SINK};
use crate::{Cut, Shared, lock};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const RETRY: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
const GRACE: Duration = Duration::from_millis(500);
const SPIN: Duration = Duration::from_millis(150);
const SPINS: u8 = 3;

pub(crate) fn seek(playbin: &gst::Element, at: gst::ClockTime, cut: Cut, flush: bool) {
    let flags =
        if flush { gst::SeekFlags::FLUSH } else { gst::SeekFlags::empty() } | gst::SeekFlags::ACCURATE;
    let sought = match cut.plain() {
        true => playbin.seek_simple(flags, at),
        false => {
            let (stop, end) = match cut.end {
                0 => (gst::SeekType::None, gst::ClockTime::NONE),
                end => (gst::SeekType::Set, Some(Cut::at(end))),
            };
            let flags = flags | gst::SeekFlags::SEGMENT;
            playbin.seek(cut.rate(), flags, gst::SeekType::Set, Some(at), stop, end)
        }
    };
    if let Err(e) = sought {
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
    pub(crate) cut: Cut,
    shared: Shared,
    mark: Option<(u64, Instant, u8)>,
    outputs: Option<Outputs>,
    delay: Duration,
    restart: Option<gst::ClockTime>,
    fresh: bool,
    hold: Option<Instant>,
}

impl Audio {
    pub(crate) fn new(uri: String, shared: Shared) -> Self {
        let cut = *lock(&shared.cut);
        Self { uri, cut, shared, fresh: true, ..Self::default() }
    }

    pub(crate) fn seek(&self, playbin: &gst::Element, at: gst::ClockTime) {
        seek(playbin, at, self.cut, true);
    }

    pub(crate) fn again(&mut self, playbin: &gst::Element, flush: bool) {
        let seen = self.shared.seen.load(Ordering::Relaxed);
        let spins = spins(self.mark, seen);
        self.mark = Some((seen, Instant::now(), spins));
        let stalled = spins >= SPINS;
        if stalled && self.cut.start == 0 && self.cut.end == 0 {
            return tracing::debug!("the video ended without a frame, it stays on its last one");
        }
        if stalled {
            tracing::info!("nothing plays between the chosen start and the end, the whole video loops");
            (self.cut.start, self.cut.end) = (0, 0);
        }
        seek(playbin, Cut::at(self.cut.start), self.cut, flush);
    }

    pub(crate) fn retarget(&mut self, playbin: &gst::Element) {
        (self.cut, self.mark) = (*lock(&self.shared.cut), None);
        if !self.fresh && self.restart.is_none() {
            seek(playbin, Cut::at(self.cut.start), self.cut, true);
        }
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
            self.rebuild(playbin, bus, pipewire::sink(None));
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
            return self.seek(playbin, at);
        }
        let piped = playbin.property::<Option<gst::Element>>("audio-sink").is_some_and(|s| s.name() == SINK);
        let fresh = std::mem::take(&mut self.fresh);
        if fresh && !self.cut.plain() {
            self.again(playbin, true);
        }
        if fresh && piped {
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

fn spins(mark: Option<(u64, Instant, u8)>, seen: u64) -> u8 {
    mark.filter(|(was, at, _)| *was == seen && at.elapsed() < SPIN).map_or(0, |(_, _, n)| n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_repeated_quick_loops_without_frames_count_as_an_empty_part() {
        let now = Instant::now();
        assert_eq!(spins(None, 1), 0);
        assert_eq!(spins(Some((1, now, 0)), 1), 1);
        assert_eq!(spins(Some((1, now, SPINS - 1)), 1), SPINS);
        assert_eq!(spins(Some((1, now, 2)), 2), 0);
        assert_eq!(spins(Some((1, now - SPIN * 3, 2)), 1), 0);
    }
}
