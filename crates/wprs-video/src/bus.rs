use crate::Event;
use crate::audio::{self, Audio};
use crate::pipewire::SINK;
use gstreamer as gst;
use gstreamer::glib::WeakRef;
use gstreamer::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const LOG_PERIOD: Duration = Duration::from_secs(5);

pub(crate) fn watch_bus(
    bus: &gst::Bus,
    (weak, uri): (&WeakRef<gst::Element>, String),
    stop: &AtomicBool,
    playing: &AtomicBool,
    sink: &dyn Fn(Event),
) {
    let (mut warned, mut audio) = (None::<Instant>, Audio::new(uri));
    while !stop.load(Ordering::Relaxed) {
        if let Some(playbin) = weak.upgrade() {
            if !audio.holding()
                && let Err(e) = apply_wanted_state(&playbin, playing.load(Ordering::Relaxed))
                && warned.is_none_or(|t| t.elapsed() >= LOG_PERIOD)
            {
                warned = Some(Instant::now());
                tracing::warn!("cannot change the video state: {e}");
            }
            audio.tick(&playbin, bus);
        }
        let Some(message) = bus.timed_pop(audio.wait()) else { continue };
        let Some(playbin) = weak.upgrade() else { continue };
        match message.view() {
            gst::MessageView::Error(err) if err.src().is_some_and(|s| s.name() == SINK) => {
                audio.failed(&playbin, bus, err)
            }
            gst::MessageView::Error(err) => {
                tracing::debug!(details = err.debug().as_deref(), "video error");
                sink(Event::Error(err.error().to_string()));
                break;
            }
            gst::MessageView::AsyncDone(_) => audio.settled(&playbin),
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

fn apply_wanted_state(playbin: &gst::Element, playing: bool) -> Result<(), gst::StateChangeError> {
    let wanted = if playing { gst::State::Playing } else { gst::State::Paused };
    let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
    let settled = pending == gst::State::VoidPending && current >= gst::State::Paused;
    if !settled || current == wanted {
        return Ok(());
    }
    playbin.set_state(wanted)?;
    if let Some(at) = playbin.query_position().filter(|_| !playing) {
        audio::seek(playbin, at);
    }
    Ok(())
}
