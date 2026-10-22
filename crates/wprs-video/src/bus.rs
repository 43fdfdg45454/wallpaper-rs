use crate::Event;
use gstreamer as gst;
use gstreamer::glib::WeakRef;
use gstreamer::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const LOG_PERIOD: Duration = Duration::from_secs(5);

pub(crate) fn watch_bus(
    bus: &gst::Bus,
    playbin: &WeakRef<gst::Element>,
    stop: &AtomicBool,
    playing: &AtomicBool,
    sink: &dyn Fn(Event),
) {
    let mut warned: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        if let Some(playbin) = playbin.upgrade()
            && let Err(e) = apply_wanted_state(&playbin, playing.load(Ordering::Relaxed))
            && warned.is_none_or(|t| t.elapsed() >= LOG_PERIOD)
        {
            warned = Some(Instant::now());
            tracing::warn!("cannot change the video state: {e}");
        }
        let Some(message) = bus.timed_pop(gst::ClockTime::NONE) else { continue };
        match message.view() {
            gst::MessageView::Error(err) => {
                tracing::debug!(details = err.debug().as_deref(), "video error");
                sink(Event::Error(err.error().to_string()));
                if let Some(playbin) = playbin.upgrade() {
                    let _ = playbin.set_state(gst::State::Null);
                }
                return;
            }
            gst::MessageView::Eos(_) => {
                if let Some(playbin) = playbin.upgrade()
                    && let Err(e) = playbin
                        .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO)
                {
                    tracing::debug!("cannot restart the video: {e}");
                }
            }
            _ => {}
        }
    }
}

fn apply_wanted_state(playbin: &gst::Element, playing: bool) -> Result<(), gst::StateChangeError> {
    let wanted = if playing { gst::State::Playing } else { gst::State::Paused };
    let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
    let settled = pending == gst::State::VoidPending && current >= gst::State::Paused;
    if !settled || current == wanted {
        return Ok(());
    }
    if playbin.set_state(wanted)? == gst::StateChangeSuccess::Success && !playing {
        let at = playbin.query_position::<gst::ClockTime>().unwrap_or_default();
        playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, at)
            .unwrap_or_else(|e| tracing::debug!("cannot settle the pause: {e}"));
    }
    Ok(())
}
