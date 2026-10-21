use crate::{Error, Event, Options, Sink, make, video_sink};
use gstreamer as gst;
use gstreamer::glib::WeakRef;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub struct Player {
    playbin: gst::Element,
    audio: bool,
    playing: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    bus_thread: Option<JoinHandle<()>>,
}

impl Player {
    pub fn open(
        path: &Path,
        options: Options,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        crate::init()?;
        let path = std::path::absolute(path).map_err(Error::path)?;
        let uri = gst::glib::filename_to_uri(&path, None).map_err(Error::path)?;
        let sink: Sink = Arc::new(sink);
        let audio = options.audible();
        let playbin = make("playbin")?;
        playbin.set_property("uri", &uri);
        playbin.set_property_from_str("flags", if audio { "video+audio+soft-volume" } else { "video" });
        playbin.set_property("volume", options.volume.clamp(0.0, 1.0));
        playbin.set_property("mute", options.muted);
        playbin.set_property("video-sink", &video_sink::build(options, sink.clone())?);
        playbin.connect("about-to-finish", false, move |args| {
            if let Ok(finishing) = args[0].get::<gst::Element>() {
                finishing.set_property("uri", &uri);
            }
            None
        });
        let (stop, playing) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(true)));
        let bus = playbin.bus().expect("playbin has a bus");
        let (weak_playbin, flags) = (playbin.downgrade(), (stop.clone(), playing.clone()));
        let bus_thread = std::thread::Builder::new()
            .name("wprs-video-bus".into())
            .spawn(move || watch_bus(&bus, &weak_playbin, &flags.0, &flags.1, &*sink))
            .map_err(Error::state)?;
        let player = Self { playbin, audio, playing, stop, bus_thread: Some(bus_thread) };
        player.playbin.set_state(gst::State::Playing).map_err(Error::state)?;
        Ok(player)
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
    }

    pub fn set_volume(&self, volume: f64, muted: bool) {
        self.playbin.set_property("volume", volume.clamp(0.0, 1.0));
        self.playbin.set_property("mute", muted);
    }

    pub fn audio_enabled(&self) -> bool {
        self.audio
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.playbin.set_state(gst::State::Null);
        if let Some(thread) = self.bus_thread.take() {
            let _ = thread.join();
        }
    }
}

fn watch_bus(
    bus: &gst::Bus,
    playbin: &WeakRef<gst::Element>,
    stop: &AtomicBool,
    playing: &AtomicBool,
    sink: &dyn Fn(Event),
) {
    while !stop.load(Ordering::Relaxed) {
        if let Some(playbin) = playbin.upgrade() {
            apply_wanted_state(&playbin, playing.load(Ordering::Relaxed));
        }
        let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else { continue };
        match message.view() {
            gst::MessageView::Error(err) => {
                let error = err.error();
                let text =
                    err.debug().map_or_else(|| error.to_string(), |debug| format!("{error} ({debug})"));
                sink(Event::Error(text));
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

fn apply_wanted_state(playbin: &gst::Element, playing: bool) {
    let wanted = if playing { gst::State::Playing } else { gst::State::Paused };
    let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
    let settled = pending == gst::State::VoidPending && current >= gst::State::Paused;
    if settled
        && current != wanted
        && let Err(e) = playbin.set_state(wanted)
    {
        tracing::warn!("cannot change the video state: {e}");
    }
}
