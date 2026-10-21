mod player;
mod video_sink;

pub use player::Player;

use gstreamer as gst;
use std::path::Path;
use std::sync::Arc;

pub const EXTENSIONS: [&str; 9] = ["mp4", "m4v", "webm", "mkv", "mov", "avi", "ogv", "mpg", "mpeg"];

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

impl Error {
    fn path(e: impl ToString) -> Self {
        Self::Path(e.to_string())
    }

    fn state(e: impl ToString) -> Self {
        Self::State(e.to_string())
    }
}

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

#[derive(Debug)]
pub enum Event {
    Frame(Frame),
    Error(String),
}

type Sink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub max_size: (u32, u32),
    pub max_fps: u32,
    pub volume: f64,
    pub muted: bool,
}

impl Options {
    fn audible(&self) -> bool {
        !self.muted && self.volume > 0.0
    }
}

pub fn init() -> Result<(), Error> {
    gst::init().map_err(|e| Error::Init(e.to_string()))
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory).build().map_err(|_| Error::MissingElement(factory))
}
