use crate::outputs::Outputs;
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

fn seek(playbin: &gst::Element, at: gst::ClockTime, cut: Cut, flags: gst::SeekFlags) {
    let flags = if cut.plain() { flags } else { flags | gst::SeekFlags::SEGMENT };
    let end = (cut.end > 0).then(|| Cut::at(cut.end));
    let (set, rate) = (gst::SeekType::Set, cut.rate());
    if let Err(e) = playbin.seek(rate, flags, set, Some(at), set, end) {
        tracing::debug!("cannot seek the video: {e}");
    }
}

fn instant(playbin: &gst::Element, cut: Cut) -> bool {
    let (none, flags) = (gst::SeekType::None, gst::SeekFlags::INSTANT_RATE_CHANGE);
    playbin.seek(cut.rate(), flags, none, gst::ClockTime::NONE, none, gst::ClockTime::NONE).is_ok()
}

pub(crate) fn from_output(src: &gst::Object) -> bool {
    let sink = |o: &gst::Object| {
        let factory = o.downcast_ref::<gst::Element>().and_then(|e| e.factory());
        o.name() == SINK || factory.is_some_and(|f| f.klass().contains("Sink/Audio"))
    };
    std::iter::successors(Some(src.clone()), gst::Object::parent).any(|o| sink(&o))
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
    aim: Option<gst::ClockTime>,
    hold: Option<Instant>,
}

impl Audio {
    pub(crate) fn new(uri: String, shared: Shared) -> Self {
        let cut = *lock(&shared.cut);
        Self { uri, cut, shared, fresh: true, ..Self::default() }
    }

    pub(crate) fn seek(&self, playbin: &gst::Element, at: gst::ClockTime, exact: bool) {
        let flags = if exact { gst::SeekFlags::ACCURATE } else { gst::SeekFlags::KEY_UNIT };
        self.jump(playbin, at, gst::SeekFlags::FLUSH | flags);
    }

    pub(crate) fn unstick(&mut self, playbin: &gst::Element, at: gst::ClockTime) {
        self.seek(playbin, at, false);
        self.restart = Some(at);
    }

    fn jump(&self, playbin: &gst::Element, at: gst::ClockTime, flags: gst::SeekFlags) {
        self.shared.seeks.fetch_add(1, Ordering::Relaxed);
        seek(playbin, at, self.cut, flags);
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
        let flush = if flush { gst::SeekFlags::FLUSH } else { gst::SeekFlags::empty() };
        self.jump(playbin, Cut::at(self.cut.start), flush | gst::SeekFlags::ACCURATE);
    }

    pub(crate) fn retarget(&mut self, playbin: &gst::Element) {
        let (was, cut) = (self.cut, *lock(&self.shared.cut));
        (self.cut, self.mark) = (cut, None);
        if self.fresh || self.restart.is_some() || was == cut {
            return;
        }
        if (was.start, was.end) != (cut.start, cut.end) {
            return self.seek(playbin, Cut::at(cut.start), true);
        }
        if !instant(playbin, cut) {
            let at = playbin.query_position::<gst::ClockTime>().unwrap_or(Cut::at(cut.start));
            self.seek(playbin, at, true);
        }
    }

    pub(crate) fn goto(&mut self, playbin: &gst::Element, at: gst::ClockTime) {
        match (self.restart.is_some(), self.fresh) {
            (true, _) => self.restart = Some(at),
            (false, true) => self.aim = Some(at),
            (false, false) => self.seek(playbin, at, true),
        }
    }

    pub(crate) fn holding(&mut self) -> bool {
        self.hold = self.hold.filter(|at| Instant::now() < *at);
        self.restart.is_some() || self.fresh || self.hold.is_some()
    }

    pub(crate) fn wait(&self) -> Option<gst::ClockTime> {
        let now = Instant::now();
        let at = self.hold.into_iter().chain(self.outputs.as_ref().and_then(Outputs::wait)).min()?;
        Some(gst::ClockTime::from_mseconds(at.saturating_duration_since(now).as_millis() as u64))
    }

    pub(crate) fn tick(&mut self, playbin: &gst::Element, bus: &gst::Bus) {
        if self.outputs.take_if(|o| o.ready(Instant::now())).is_some() {
            tracing::info!("trying the audio output again");
            self.rebuild(playbin, bus, pipewire::sink(self.shared.latency));
        }
    }

    pub(crate) fn failed(&mut self, playbin: &gst::Element, bus: &gst::Bus, err: &gst::message::Error) {
        tracing::debug!(details = err.debug().as_deref(), "audio output error");
        let piped = playbin.property::<Option<gst::Element>>("audio-sink").is_some_and(|s| s.name() == SINK);
        let reached = err.error().message().starts_with("stream error");
        if piped && !reached && std::env::var_os("FLATPAK_ID").is_none() {
            tracing::info!("cannot reach PipeWire, using the default audio output");
            return self.rebuild(playbin, bus, None);
        }
        tracing::info!("no audio output, the picture goes on without sound until one appears");
        self.delay = (self.delay * 2).clamp(RETRY, RETRY_MAX);
        self.outputs = Some(Outputs::watch(bus, Instant::now() + self.delay));
        let silent = gst::ElementFactory::make("fakesink").property("sync", true).build().ok();
        self.rebuild(playbin, bus, silent);
    }

    pub(crate) fn settled(&mut self, playbin: &gst::Element) {
        if let Some(at) = self.restart.take() {
            return self.seek(playbin, at, true);
        }
        let piped = playbin.property::<Option<gst::Element>>("audio-sink").is_some_and(|s| s.name() == SINK);
        let (fresh, aim) = (std::mem::take(&mut self.fresh), self.aim.take());
        match aim.filter(|_| fresh) {
            Some(at) => self.seek(playbin, at, true),
            None if fresh && !self.cut.plain() => self.again(playbin, true),
            None => {}
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
mod tests;
