use crate::bus::Pipe;
use crate::{Cut, Error, Event, Options, Shared, Sink, local_uri, lock, make, pipewire, video_sink};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::Arc;

pub struct Player {
    pipe: Pipe,
    size: gst::Element,
    rate: gst::Element,
    audio: bool,
    shared: Shared,
    max_fps: u32,
    zero_copy: bool,
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
        let piped = audio.then(|| pipewire::sink(None)).flatten();
        piped.iter().for_each(|sink| playbin.set_property("audio-sink", sink));
        let tempo = audio.then(|| make("scaletempo").ok()).flatten();
        tempo.iter().for_each(|tempo| playbin.set_property("audio-filter", tempo));
        let shared = Shared { cut: Arc::new(options.cut.into()), ..Shared::default() };
        let (video_sink, size, rate, zero_copy) =
            video_sink::build(options, sink.clone(), shared.seen.clone())?;
        playbin.set_property("video-sink", &video_sink);
        if zero_copy {
            let flags = if audio { "video+audio+soft-volume+native-video" } else { "video+native-video" };
            playbin.set_property_from_str("flags", flags);
        }
        let (again, cut, once) = (uri.clone(), shared.cut.clone(), shared.once.clone());
        if !audio {
            playbin.connect("about-to-finish", false, move |args| {
                let looping = lock(&cut).plain() && !once.load(std::sync::atomic::Ordering::Relaxed);
                if let Some(finishing) = args[0].get::<gst::Element>().ok().filter(|_| looping) {
                    finishing.set_property("uri", &again);
                }
                None
            });
        }
        let pipe = Pipe::start(playbin, (uri, shared.clone()), sink, audio)?;
        let max_fps = options.max_fps;
        Ok(Self { pipe, size, rate, audio, shared, max_fps, zero_copy })
    }

    pub fn set_cut(&mut self, cut: Cut) {
        *lock(&self.shared.cut) = cut;
        if self.max_fps > 0 {
            video_sink::set_max_fps(&self.rate, self.max_fps, cut);
        }
        self.pipe.post("wprs-cut");
    }

    pub fn play_once(&self, once: bool) {
        self.shared.once.store(once, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn seek(&self, seconds: f64) {
        let at = (seconds.clamp(0.0, 86_400.0) * 1e9) as u64;
        self.pipe.send(gst::Structure::builder("wprs-seek").field("at", at).build());
    }

    pub fn set_playing(&self, playing: bool) {
        self.pipe.set_playing(playing);
    }

    pub fn paused(&self) -> bool {
        self.pipe.paused()
    }

    pub fn set_max_size(&self, max_size: (u32, u32)) {
        self.size.set_property("caps", video_sink::size_caps(max_size, self.zero_copy));
    }

    pub fn set_max_fps(&mut self, max_fps: u32) {
        self.max_fps = max_fps;
        video_sink::set_max_fps(&self.rate, max_fps, *lock(&self.shared.cut));
    }

    pub fn set_volume(&self, volume: f64, muted: bool) {
        self.pipe.playbin.set_property("volume", volume.clamp(0.0, 1.0));
        self.pipe.playbin.set_property("mute", muted);
    }

    pub fn seeks(&self) -> u64 {
        self.shared.seeks.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn audio_enabled(&self) -> bool {
        self.audio
    }
}
