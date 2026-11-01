use crate::audio::{Audio, from_output};
use crate::{Error, Event, Shared, Sink};
use gstreamer as gst;
use gstreamer::glib::WeakRef;
use gstreamer::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

const LOG_PERIOD: Duration = Duration::from_secs(5);
const STOP_WAIT: Duration = Duration::from_secs(2);
const STUCK: Duration = Duration::from_secs(1);

pub(crate) struct Pipe {
    pub(crate) playbin: gst::Element,
    playing: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    stopped: Receiver<()>,
}

impl Pipe {
    pub(crate) fn start(
        playbin: gst::Element,
        (uri, shared): (String, Shared),
        sink: Sink,
        audible: bool,
    ) -> Result<Pipe, Error> {
        let (stop, playing) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(true)));
        let bus = playbin.bus().expect("playbin has a bus");
        let (weak, flags) = (playbin.downgrade(), (stop.clone(), playing.clone()));
        let (done, stopped) = mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("wprs-video-bus".into())
            .spawn(move || {
                let _done = done;
                watch_bus(&bus, (&weak, uri, shared), &flags.0, &flags.1, &*sink);
            })
            .map_err(Error::state)?;
        let pipe = Pipe { playbin, playing, stop, stopped };
        match pipe.playbin.set_state(gst::State::Paused) {
            Err(_) if audible => {}
            started => drop(started.map_err(Error::state)?),
        }
        Ok(pipe)
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        self.wake();
    }

    pub(crate) fn paused(&self) -> bool {
        let (_, current, pending) = self.playbin.state(gst::ClockTime::ZERO);
        current == gst::State::Paused && pending == gst::State::VoidPending
    }

    fn wake(&self) {
        self.post("wprs-wake");
    }

    pub(crate) fn post(&self, name: &str) {
        self.send(gst::Structure::new_empty(name));
    }

    pub(crate) fn send(&self, message: gst::Structure) {
        if let Some(bus) = self.playbin.bus() {
            bus.set_flushing(false);
            let _ = bus.post(gst::message::Application::new(message));
        }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake();
        if let Err(RecvTimeoutError::Timeout) = self.stopped.recv_timeout(STOP_WAIT) {
            tracing::warn!("playback did not stop within {STOP_WAIT:?}, leaving it behind");
        }
    }
}

pub(crate) fn watch_bus(
    bus: &gst::Bus,
    (weak, uri, shared): (&WeakRef<gst::Element>, String, Shared),
    stop: &AtomicBool,
    playing: &AtomicBool,
    sink: &dyn Fn(Event),
) {
    let (mut warned, mut audio, mut pausing) = (None::<Instant>, Audio::new(uri, shared), None::<Instant>);
    while !stop.load(Ordering::Relaxed) {
        if let Some(playbin) = weak.upgrade() {
            let wanted = playing.load(Ordering::Relaxed);
            match (!audio.holding()).then(|| apply_wanted_state(&playbin, wanted)) {
                Some(Ok(true)) => pausing = Some(Instant::now() + STUCK),
                Some(Err(e)) if warned.is_none_or(|t| t.elapsed() >= LOG_PERIOD) => {
                    warned = Some(Instant::now());
                    tracing::warn!("cannot change the video state: {e}");
                }
                _ => {}
            }
            pausing = pausing.filter(|_| !wanted);
            if pausing.take_if(|at| *at <= Instant::now()).is_some() {
                unstick(&playbin, &mut audio);
            }
            audio.tick(&playbin, bus);
        }
        let wait = pausing.map(|at| gst::ClockTime::from_mseconds(remaining(at)));
        let Some(message) = bus.timed_pop(audio.wait().into_iter().chain(wait).min()) else { continue };
        let Some(playbin) = weak.upgrade() else { continue };
        match message.view() {
            gst::MessageView::Error(err) if err.src().is_some_and(from_output) => {
                audio.failed(&playbin, bus, err)
            }
            gst::MessageView::Error(err) => {
                tracing::debug!(details = err.debug().as_deref(), "video error");
                sink(Event::Error(err.error().to_string()));
                break;
            }
            gst::MessageView::AsyncDone(_) => audio.settled(&playbin),
            gst::MessageView::Application(a) => match a.structure() {
                Some(s) if s.name() == "wprs-cut" => audio.retarget(&playbin),
                Some(s) if s.name() == "wprs-seek" => {
                    let at = s.get::<u64>("at").map(gst::ClockTime::from_nseconds);
                    at.into_iter().for_each(|at| audio.goto(&playbin, at));
                }
                _ => {}
            },
            gst::MessageView::SegmentDone(_) => audio.again(&playbin, false),
            gst::MessageView::Eos(_) if !audio.cut.plain() => audio.again(&playbin, true),
            gst::MessageView::Eos(_) if audio.once() => {}
            gst::MessageView::Eos(_) => {
                let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
                if let Err(e) = playbin.seek_simple(flags, gst::ClockTime::ZERO) {
                    tracing::debug!("cannot restart the video: {e}");
                }
            }
            _ => {}
        }
    }
    if let Some(playbin) = weak.upgrade() {
        let _ = playbin.set_state(gst::State::Null);
    }
}

fn apply_wanted_state(playbin: &gst::Element, playing: bool) -> Result<bool, gst::StateChangeError> {
    let wanted = if playing { gst::State::Playing } else { gst::State::Paused };
    let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
    let settled = pending == gst::State::VoidPending && current >= gst::State::Paused;
    if !settled || current == wanted {
        return Ok(false);
    }
    playbin.set_state(wanted).map(|_| !playing)
}

fn unstick(playbin: &gst::Element, audio: &mut Audio) {
    let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
    let settled = pending == gst::State::VoidPending && current == gst::State::Paused;
    if let Some(at) = playbin.query_position().filter(|_| !settled) {
        tracing::debug!("the pause did not settle, finishing it at a key frame and then on the exact frame");
        audio.unstick(playbin, at);
    }
}

fn remaining(at: Instant) -> u64 {
    at.saturating_duration_since(Instant::now()).as_millis() as u64
}
