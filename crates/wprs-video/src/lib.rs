mod bus;
mod player;
mod probe;
mod video_sink;

pub use player::Player;
pub use probe::probe;

use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::{Arc, OnceLock};

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
    #[error("{0} is not an existing local file")]
    NotLocal(String),
    #[error("cannot decode the video: {0}")]
    Decode(String),
    #[error("cannot start playback: {0}")]
    State(String),
}

impl Error {
    fn state(e: impl ToString) -> Self {
        Self::State(e.to_string())
    }
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    offset: usize,
    buffer: gst::MappedBuffer<gst::buffer::Readable>,
}

impl Frame {
    pub fn data(&self) -> &[u8] {
        &self.buffer.as_slice()[self.offset..]
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
    static READY: OnceLock<Result<(), String>> = OnceLock::new();
    let ready = READY.get_or_init(|| gst::init().map(|()| local_sources_only()).map_err(|e| e.to_string()));
    ready.clone().map_err(Error::Init)
}

fn local_sources_only() {
    let registry = gst::Registry::get();
    let remote = |feature: &gst::PluginFeature| {
        feature.downcast_ref::<gst::ElementFactory>().is_some_and(|f| {
            let fetches = f.uri_type() == gst::URIType::Src && f.uri_protocols().iter().any(|p| p != "file");
            fetches || ["Network", "Adaptive"].iter().any(|k| f.klass().contains(k))
        })
    };
    registry.features_filtered(remote, false).iter().for_each(|f| registry.remove_feature(f));
}

fn local_uri(path: &Path) -> Result<String, Error> {
    let not_local = || Error::NotLocal(path.display().to_string());
    let absolute = std::path::absolute(path).map_err(|_| not_local())?;
    absolute.is_file().then_some(()).ok_or_else(not_local)?;
    gst::glib::filename_to_uri(&absolute, None).map(Into::into).map_err(|_| not_local())
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory).build().map_err(|_| Error::MissingElement(factory))
}
