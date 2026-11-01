mod audio;
mod bank;
mod bus;
mod dmabuf;
mod feed;
mod mix;
mod outputs;
mod pipewire;
mod player;
mod probe;
mod sound;
mod still;
mod video_sink;

pub use bank::{Bank, decode, probe as probe_sound};
pub use dmabuf::{Dmabuf, accept as accept_dmabuf};
pub use mix::{Clip, Mix, RATE, Source, Track};
pub use player::Player;
pub use probe::probe;
pub use sound::Soundtrack;
pub use still::{Still, still};

use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

pub const UNSHARED: &str = "cannot share this video frame with the GPU";
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
    #[error("the video is not an existing local file")]
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
    pub pts: Option<Duration>,
    pixels: Pixels,
}

#[derive(Debug)]
enum Pixels {
    Mapped(usize, gst::MappedBuffer<gst::buffer::Readable>),
    Shared { shared: Dmabuf, _held: gst::Buffer },
}

impl Frame {
    pub fn data(&self) -> &[u8] {
        match &self.pixels {
            Pixels::Mapped(offset, buffer) => &buffer.as_slice()[*offset..],
            Pixels::Shared { .. } => &[],
        }
    }

    pub fn dmabuf(&self) -> Option<&Dmabuf> {
        match &self.pixels {
            Pixels::Shared { shared, .. } => Some(shared),
            Pixels::Mapped(..) => None,
        }
    }
}

#[derive(Debug)]
pub enum Event {
    Frame(Frame),
    Error(String),
}

type Sink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) struct Shared {
    pub(crate) cut: Arc<Mutex<Cut>>,
    pub(crate) seen: Arc<AtomicU64>,
    pub(crate) seeks: Arc<AtomicU64>,
    pub(crate) once: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) latency: Option<&'static str>,
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub max_size: (u32, u32),
    pub max_fps: u32,
    pub volume: f64,
    pub muted: bool,
    pub cut: Cut,
    pub zero_copy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cut {
    pub speed: u32,
    pub start: u32,
    pub end: u32,
}

impl Default for Cut {
    fn default() -> Self {
        Cut { speed: 100, start: 0, end: 0 }
    }
}

impl Cut {
    pub fn plain(&self) -> bool {
        *self == Cut::default()
    }

    fn rate(&self) -> f64 {
        f64::from(self.speed.clamp(10, 400)) / 100.0
    }

    fn cap(&self, max_fps: u32) -> i32 {
        (u64::from(max_fps) * 100 / u64::from(self.speed.clamp(10, 400))).clamp(1, i32::MAX as u64) as i32
    }

    fn at(ms: u32) -> gst::ClockTime {
        gst::ClockTime::from_mseconds(u64::from(ms))
    }
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
            let fetches = f.uri_type() == gst::URIType::Src
                && f.uri_protocols().iter().any(|p| !["file", "appsrc"].contains(&p.as_str()));
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

pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let (path, host) = gst::glib::filename_from_uri(uri).ok()?;
    host.is_none_or(|host| host == "localhost").then_some(path)
}

fn make(factory: &'static str) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(factory).build().map_err(|_| Error::MissingElement(factory))
}
