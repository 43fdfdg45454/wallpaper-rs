//! Looping video playback for wallpapers.
//!
//! A [`Player`] decodes a file with GStreamer's `playbin`, converts frames to
//! RGBA no larger than the outputs that show them, and hands each frame to a
//! callback on a GStreamer streaming thread. Playback loops without a gap
//! and can be paused; audio is only decoded while it is audible.

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

/// File extensions treated as videos.
pub const EXTENSIONS: [&str; 9] = ["mp4", "m4v", "webm", "mkv", "mov", "avi", "ogv", "mpg", "mpeg"];

/// Whether `path` looks like a video file, judging by its extension.
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot initialize GStreamer: {0}")]
    Init(String),
    #[error("GStreamer element '{0}' is missing; install the GStreamer base plugins")]
    MissingElement(&'static str),
    #[error("invalid video path {0}")]
    Path(String),
    #[error("cannot start playback: {0}")]
    State(String),
}

/// A decoded frame: RGBA, rows of `stride` bytes.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .finish()
    }
}

/// What a player reports.
#[derive(Debug)]
pub enum Event {
    Frame(Frame),
    /// Playback failed and stopped (unsupported codec, unreadable file...).
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Frames are scaled down, keeping the aspect ratio, to fit this size.
    pub max_size: (u32, u32),
    /// Frames beyond this rate are dropped before conversion.
    pub max_fps: u32,
    /// Linear volume, 0.0 to 1.0.
    pub volume: f64,
    pub muted: bool,
}

impl Options {
    fn audible(&self) -> bool {
        !self.muted && self.volume > 0.0
    }
}

/// Initializes GStreamer once per process.
pub fn init() -> Result<(), Error> {
    gst::init().map_err(|e| Error::Init(e.to_string()))
}

/// A looping video. Dropping it stops playback.
pub struct Player {
    playbin: gst::Element,
    audio: bool,
    stop: Arc<AtomicBool>,
    bus_thread: Option<JoinHandle<()>>,
}

impl Player {
    /// Opens `path` and starts playing. Frames and errors go to `sink`.
    pub fn open(
        path: &Path,
        options: Options,
        sink: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        init()?;
        let path = std::path::absolute(path).map_err(|e| Error::Path(e.to_string()))?;
        let uri = gst::glib::filename_to_uri(&path, None).map_err(|e| Error::Path(e.to_string()))?;
        let sink = Arc::new(sink);

        let playbin = make("playbin")?;
        playbin.set_property("uri", &uri);
        // Audio is only decoded when it can be heard; enabling it later needs
        // a new player (see `audio_enabled`).
        let audio = options.audible();
        playbin.set_property_from_str("flags", if audio { "video+audio+soft-volume" } else { "video" });
        playbin.set_property("volume", options.volume.clamp(0.0, 1.0));
        playbin.set_property("mute", options.muted);
        playbin.set_property("video-sink", &video_sink(options, sink.clone())?);

        // Queue the same file again just before the end: gapless looping.
        playbin.connect("about-to-finish", false, move |args| {
            if let Ok(pb) = args[0].get::<gst::Element>() {
                pb.set_property("uri", &uri);
            }
            None
        });

        let stop = Arc::new(AtomicBool::new(false));
        let bus = playbin.bus().expect("playbin has a bus");
        let bus_thread = {
            let stop = stop.clone();
            let weak = playbin.downgrade();
            let sink = sink.clone();
            std::thread::Builder::new()
                .name("wprs-video-bus".into())
                .spawn(move || watch_bus(&bus, &weak, &stop, sink.as_ref()))
                .map_err(|e| Error::State(e.to_string()))?
        };

        let player = Self {
            playbin,
            audio,
            stop,
            bus_thread: Some(bus_thread),
        };
        player
            .playbin
            .set_state(gst::State::Playing)
            .map_err(|e| Error::State(e.to_string()))?;
        Ok(player)
    }

    /// Resumes or pauses playback. A paused player keeps its last frame.
    pub fn set_playing(&self, playing: bool) {
        let state = if playing {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        // Changing the state while an asynchronous change is still in
        // flight (right after opening) can leave the pipeline stuck in
        // "PAUSED, pending PLAYING": let the pending change finish first.
        let _ = self.playbin.state(gst::ClockTime::from_seconds(1));
        if let Err(e) = self.playbin.set_state(state) {
            tracing::warn!("cannot change the video state: {e}");
        }
    }

    /// Changes the volume. Making a player that was opened silent audible
    /// requires opening a new one: check [`Player::audio_enabled`].
    pub fn set_volume(&self, volume: f64, muted: bool) {
        self.playbin.set_property("volume", volume.clamp(0.0, 1.0));
        self.playbin.set_property("mute", muted);
    }

    /// Whether the audio stream is decoded at all.
    pub fn audio_enabled(&self) -> bool {
        self.audio
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.playbin.set_state(gst::State::Null);
        if let Some(t) = self.bus_thread.take() {
            let _ = t.join();
        }
    }
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| Error::MissingElement(factory))
}

/// `videorate ! videoconvert ! videoscale ! appsink(RGBA, at most max_size)`.
fn video_sink(options: Options, sink: Arc<dyn Fn(Event) + Send + Sync>) -> Result<gst::Element, Error> {
    let bin = gst::Bin::with_name("wprs-video-sink");
    let rate = make("videorate")?;
    if options.max_fps > 0 {
        rate.set_property("max-rate", options.max_fps.min(i32::MAX as u32) as i32);
        if rate.find_property("drop-only").is_some() {
            rate.set_property("drop-only", true);
        }
    }
    let convert = make("videoconvert")?;
    let scale = make("videoscale")?;
    let (max_w, max_h) = options.max_size;
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .field(
            "width",
            gst::IntRange::new(1, max_w.clamp(1, i32::MAX as u32) as i32),
        )
        .field(
            "height",
            gst::IntRange::new(1, max_h.clamp(1, i32::MAX as u32) as i32),
        )
        .build();
    let appsink = gst_app::AppSink::builder()
        .caps(&caps)
        .sync(true)
        .max_buffers(1)
        .drop(true)
        .enable_last_sample(false)
        .build();
    let on_sample = sink.clone();
    let on_preroll = sink;
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |s| {
                let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                if let Some(frame) = to_frame(&sample) {
                    on_sample(Event::Frame(frame));
                }
                Ok(gst::FlowSuccess::Ok)
            })
            // The first frame also arrives while paused, so a wallpaper that
            // starts paused still shows its first frame.
            .new_preroll(move |s| {
                let sample = s.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                if let Some(frame) = to_frame(&sample) {
                    on_preroll(Event::Frame(frame));
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    let appsink: gst::Element = appsink.upcast();
    bin.add_many([&rate, &convert, &scale, &appsink])
        .map_err(|e| Error::State(e.to_string()))?;
    gst::Element::link_many([&rate, &convert, &scale, &appsink]).map_err(|e| Error::State(e.to_string()))?;
    let pad = rate.static_pad("sink").expect("videorate has a sink pad");
    let ghost = gst::GhostPad::with_target(&pad).map_err(|e| Error::State(e.to_string()))?;
    bin.add_pad(&ghost).map_err(|e| Error::State(e.to_string()))?;
    Ok(bin.upcast())
}

fn to_frame(sample: &gst::Sample) -> Option<Frame> {
    let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    let stride = u32::try_from(*info.stride().first()?).ok()?;
    let offset = *info.offset().first()?;
    let (width, height) = (info.width(), info.height());
    let len = stride as usize * height as usize;
    let data = map.as_slice().get(offset..offset + len)?.to_vec();
    Some(Frame {
        width,
        height,
        stride,
        data,
    })
}

fn watch_bus(
    bus: &gst::Bus,
    playbin: &gst::glib::WeakRef<gst::Element>,
    stop: &AtomicBool,
    sink: &(dyn Fn(Event) + Send + Sync),
) {
    while !stop.load(Ordering::Relaxed) {
        let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else {
            continue;
        };
        match msg.view() {
            gst::MessageView::Error(err) => {
                let text = match err.debug() {
                    Some(debug) => format!("{} ({debug})", err.error()),
                    None => err.error().to_string(),
                };
                sink(Event::Error(text));
                if let Some(pb) = playbin.upgrade() {
                    let _ = pb.set_state(gst::State::Null);
                }
                return;
            }
            // Gapless looping failed (some containers): start over.
            gst::MessageView::Eos(_) => {
                if let Some(pb) = playbin.upgrade()
                    && let Err(e) = pb.seek_simple(
                        gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                        gst::ClockTime::ZERO,
                    )
                {
                    tracing::debug!("cannot restart the video: {e}");
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_video_extensions() {
        assert!(is_video(Path::new("/a/b.MP4")));
        assert!(is_video(Path::new("clip.webm")));
        assert!(!is_video(Path::new("image.png")));
        assert!(!is_video(Path::new("noext")));
    }
}
