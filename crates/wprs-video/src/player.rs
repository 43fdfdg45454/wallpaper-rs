use crate::bus::watch_bus;
use crate::{Error, Event, Options, Sink, local_uri, make, video_sink};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub struct Player {
    playbin: gst::Element,
    size: gst::Element,
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
        let uri = local_uri(path)?;
        let sink: Sink = Arc::new(sink);
        let audio = options.audible();
        let playbin = make("playbin")?;
        playbin.set_property("uri", &uri);
        playbin.set_property_from_str("flags", if audio { "video+audio+soft-volume" } else { "video" });
        playbin.set_property("volume", options.volume.clamp(0.0, 1.0));
        playbin.set_property("mute", options.muted);
        let (video_sink, size) = video_sink::build(options, sink.clone())?;
        playbin.set_property("video-sink", &video_sink);
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
        let player = Self { playbin, size, audio, playing, stop, bus_thread: Some(bus_thread) };
        player.playbin.set_state(gst::State::Playing).map_err(Error::state)?;
        Ok(player)
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        self.wake();
    }

    pub fn paused(&self) -> bool {
        let (_, current, pending) = self.playbin.state(gst::ClockTime::ZERO);
        current == gst::State::Paused && pending == gst::State::VoidPending
    }

    fn wake(&self) {
        if let Some(bus) = self.playbin.bus() {
            bus.set_flushing(false);
            let _ = bus.post(gst::message::Application::new(gst::Structure::new_empty("wprs-wake")));
        }
    }

    pub fn set_max_size(&self, max_size: (u32, u32)) {
        self.size.set_property("caps", video_sink::size_caps(max_size));
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
        self.wake();
        if let Some(thread) = self.bus_thread.take() {
            let _ = thread.join();
        }
        let _ = self.playbin.set_state(gst::State::Null);
    }
}
