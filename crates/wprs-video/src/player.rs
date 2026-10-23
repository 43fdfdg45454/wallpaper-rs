use crate::bus::Pipe;
use crate::{Error, Event, Options, Sink, local_uri, make, pipewire, video_sink};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::Arc;

pub struct Player {
    pipe: Pipe,
    size: gst::Element,
    rate: gst::Element,
    audio: bool,
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
        let piped = audio.then(pipewire::sink).flatten();
        piped.iter().for_each(|sink| playbin.set_property("audio-sink", sink));
        let (video_sink, size, rate) = video_sink::build(options, sink.clone())?;
        playbin.set_property("video-sink", &video_sink);
        let again = uri.clone();
        if !audio {
            playbin.connect("about-to-finish", false, move |args| {
                if let Ok(finishing) = args[0].get::<gst::Element>() {
                    finishing.set_property("uri", &again);
                }
                None
            });
        }
        let pipe = Pipe::start(playbin, uri, sink, piped.is_some())?;
        Ok(Self { pipe, size, rate, audio })
    }

    pub fn set_playing(&self, playing: bool) {
        self.pipe.set_playing(playing);
    }

    pub fn paused(&self) -> bool {
        self.pipe.paused()
    }

    pub fn set_max_size(&self, max_size: (u32, u32)) {
        self.size.set_property("caps", video_sink::size_caps(max_size));
    }

    pub fn set_max_fps(&self, max_fps: u32) {
        video_sink::set_max_fps(&self.rate, max_fps);
    }

    pub fn set_volume(&self, volume: f64, muted: bool) {
        self.pipe.playbin.set_property("volume", volume.clamp(0.0, 1.0));
        self.pipe.playbin.set_property("mute", muted);
    }

    pub fn audio_enabled(&self) -> bool {
        self.audio
    }
}
