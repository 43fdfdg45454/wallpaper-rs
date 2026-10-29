use crate::mix::RATE;
use crate::{Error, local_uri, lock, make};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use std::path::Path;
use std::sync::{Arc, Mutex};

const NEAR: u64 = RATE as u64 / 4;
const QUEUED: u32 = 8;
const OPEN_WAIT: u64 = 5;

pub(crate) fn raw(mono: bool) -> gst::Caps {
    let caps = gst::Caps::builder("audio/x-raw").field("format", "S16LE").field("layout", "interleaved");
    let caps = caps.field("rate", RATE as i32);
    if mono { caps.field("channels", gst::IntRange::new(1, 2)) } else { caps.field("channels", 2) }.build()
}

pub(crate) fn decoder(path: &Path, queued: Option<u32>) -> Result<(gst::Pipeline, AppSink), Error> {
    crate::init()?;
    let decoder = gst::ElementFactory::make("uridecodebin").property("uri", local_uri(path)?).build();
    let decoder = decoder.map_err(|_| Error::MissingElement("uridecodebin"))?;
    let (convert, resample) = (make("audioconvert")?, make("audioresample")?);
    let caps = raw(true);
    let sink = AppSink::builder().caps(&caps).sync(false);
    let sink = match queued {
        Some(n) => sink.max_buffers(n).drop(false).build(),
        None => sink.build(),
    };
    let pipeline = gst::Pipeline::new();
    pipeline.add_many([&decoder, &convert, &resample, sink.upcast_ref()]).map_err(Error::state)?;
    gst::Element::link_many([&convert, &resample, sink.upcast_ref()]).map_err(Error::state)?;
    let input = convert.static_pad("sink").expect("audioconvert has a sink pad");
    decoder.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
        if caps.structure(0).is_some_and(|s| s.name().starts_with("audio/")) && !input.is_linked() {
            let _ = pad.link(&input);
        }
    });
    Ok((pipeline, sink))
}

fn to_time(frame: u64) -> gst::ClockTime {
    gst::ClockTime::from_nseconds(frame * 1_000_000_000 / u64::from(RATE))
}

pub(crate) struct Feed {
    pipeline: gst::Pipeline,
    sink: AppSink,
    samples: Vec<i16>,
    channels: usize,
    first: u64,
    asked: Option<u64>,
}

pub(crate) enum Slot {
    Waiting(Arc<Mutex<Option<Feed>>>),
    Ready(Feed),
}

impl Slot {
    pub(crate) fn open(path: &Path, from: u64) -> Slot {
        let (shared, path) = (Arc::new(Mutex::new(None)), path.to_owned());
        let filled = shared.clone();
        let opened = std::thread::Builder::new().name("wprs-sound-feed".into()).spawn(move || {
            match Feed::start(&path, from) {
                Ok(feed) => *lock(&filled) = Some(feed),
                Err(e) => tracing::debug!(path = %path.display(), "cannot stream a sound: {e}"),
            }
        });
        opened.map(drop).unwrap_or_else(|e| tracing::warn!("cannot start streaming a sound: {e}"));
        Slot::Waiting(shared)
    }

    pub(crate) fn settle(&mut self) {
        if let Slot::Waiting(shared) = self
            && let Some(feed) = shared.try_lock().ok().and_then(|mut f| f.take())
        {
            *self = Slot::Ready(feed);
        }
    }

    pub(crate) fn frame(&mut self, at: u64) -> Option<[i16; 2]> {
        match self {
            Slot::Ready(feed) => feed.frame(at),
            Slot::Waiting(_) => None,
        }
    }
}

impl Feed {
    fn start(path: &Path, from: u64) -> Result<Feed, Error> {
        let (pipeline, sink) = decoder(path, Some(QUEUED))?;
        pipeline.set_state(gst::State::Paused).map_err(Error::state)?;
        let (settled, ..) = pipeline.state(gst::ClockTime::from_seconds(OPEN_WAIT));
        settled.map_err(Error::state)?;
        if from > 0 {
            let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE;
            pipeline.seek_simple(flags, to_time(from)).map_err(Error::state)?;
        }
        pipeline.set_state(gst::State::Playing).map_err(Error::state)?;
        let asked = (from > 0).then_some(from);
        Ok(Feed { pipeline, sink, samples: Vec::new(), channels: 1, first: 0, asked })
    }

    fn len(&self) -> u64 {
        (self.samples.len() / self.channels) as u64
    }

    fn frame(&mut self, at: u64) -> Option<[i16; 2]> {
        loop {
            if (self.first..self.first + self.len()).contains(&at) {
                let i = (at - self.first) as usize * self.channels;
                return Some([self.samples[i], self.samples[i + self.channels - 1]]);
            }
            let end = self.first + self.len();
            let far = at + NEAR < self.first || at >= end + NEAR;
            if self.asked.is_none() && far {
                self.seek(at);
            }
            if self.asked.is_none() && at < self.first {
                return None;
            }
            let sample = self.sink.try_pull_sample(gst::ClockTime::ZERO)?;
            let buffer = sample.buffer()?;
            let first = buffer
                .pts()
                .map_or(self.first + self.len(), |t| t.nseconds() * u64::from(RATE) / 1_000_000_000);
            if self.asked.is_some_and(|asked| first + NEAR < asked || first > asked + NEAR) {
                continue;
            }
            let channels = sample.caps().and_then(|c| c.structure(0)?.get::<i32>("channels").ok());
            let map = buffer.map_readable().ok()?;
            self.samples = map.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
            (self.first, self.channels, self.asked) =
                (first, channels.unwrap_or(1).clamp(1, 2) as usize, None);
        }
    }

    fn seek(&mut self, at: u64) {
        self.asked = Some(at);
        let pipeline = self.pipeline.downgrade();
        let sought = std::thread::Builder::new().name("wprs-sound-seek".into()).spawn(move || {
            let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE;
            pipeline.upgrade().map(|p| p.seek_simple(flags, to_time(at)))
        });
        sought.map(drop).unwrap_or_else(|e| tracing::debug!("cannot seek a streamed sound: {e}"));
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        let pipeline = self.pipeline.clone();
        let stopped = std::thread::Builder::new().name("wprs-sound-stop".into()).spawn(move || {
            let _ = pipeline.set_state(gst::State::Null);
        });
        if stopped.is_err() {
            let _ = self.pipeline.set_state(gst::State::Null);
        }
    }
}
