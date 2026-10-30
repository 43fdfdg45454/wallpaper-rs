mod feed;
mod frames;
mod guard;
mod iio;
mod motion;
mod output;
mod parallax;
mod pictures;
mod players;
mod power;
mod scene;
mod slot;
mod speakers;
mod state;
pub(crate) mod stops;
mod surface;

use crate::fatal;
use crate::scenes::ScenePack;
use image::RgbaImage;
use state::State;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use wprs_core::{Edit, Fit, Place};
use wprs_pipewire::Exporter;
use wprs_render::{DepthMap, RenderError, Renderer, TargetLimits};
use wprs_scene::{AudioFrame, PointerState, ScriptOutputs, Values};
use wprs_video::Cut;
use wprs_wayland::Window;

#[derive(Clone)]
pub enum Content {
    Color,
    Image(Picture),
    Video(Movie),
    Scene(SceneSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reel {
    pub path: PathBuf,
    pub cut: Cut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Movie {
    pub reel: Reel,
    pub volume: u32,
    pub zoom: f32,
    pub pan: [f32; 2],
}

#[derive(Clone)]
pub struct SceneSpec {
    pub pack: Arc<ScenePack>,
    pub values: Values,
    pub extra: Vec<(String, Arc<wprs_render::Pixels>)>,
    pub clips: Vec<(String, Reel)>,
    pub hold: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum SceneInput {
    Audio(String, Box<AudioFrame>),
    Pointer(String, PointerState),
    Script(String, ScriptOutputs),
}

#[derive(Clone)]
pub struct Picture {
    pub file: PathBuf,
    pub modified: Option<SystemTime>,
    pub pixels: Option<Arc<RgbaImage>>,
    pub edit: Option<Arc<Edit>>,
    pub place: Place,
    pub depth: Option<Depth>,
}

#[derive(Clone)]
pub struct Depth {
    pub file: PathBuf,
    pub modified: Option<SystemTime>,
    pub map: Arc<DepthMap>,
}

impl Content {
    fn same_source(&self, other: &Content) -> bool {
        match (self, other) {
            (Content::Color, Content::Color) => true,
            (Content::Image(a), Content::Image(b)) => (&a.file, a.modified) == (&b.file, b.modified),
            (Content::Video(a), Content::Video(b)) => a.reel.path == b.reel.path,
            (Content::Scene(a), Content::Scene(b)) => Arc::ptr_eq(&a.pack, &b.pack),
            _ => false,
        }
    }

    fn same_look(&self, other: &Content) -> bool {
        let look = |c: &Content| match c {
            Content::Image(p) => {
                let depth = p.depth.as_ref().map(|d| (d.file.clone(), d.modified));
                Some((p.edit.clone(), p.place.clone(), depth))
            }
            _ => None,
        };
        let values = |c: &Content| match c {
            Content::Scene(s) => Some((s.values.clone(), s.media(), s.hold)),
            _ => None,
        };
        let frame = |c: &Content| match c {
            Content::Video(m) => Some((m.reel.cut, m.volume, m.zoom, m.pan)),
            _ => None,
        };
        let same = look(self) == look(other) && values(self) == values(other) && frame(self) == frame(other);
        self.same_source(other) && same
    }

    fn video(&self) -> Option<&Reel> {
        match self {
            Content::Video(movie) => Some(&movie.reel),
            _ => None,
        }
    }

    fn videos(&self) -> Vec<(&Reel, u32)> {
        match self {
            Content::Video(movie) => vec![(&movie.reel, movie.volume)],
            Content::Scene(s) => s.reels().map(|(_, reel, volume)| (reel, volume)).collect(),
            _ => Vec::new(),
        }
    }
}

impl Reel {
    pub fn plain(path: PathBuf) -> Reel {
        Reel { path, cut: Cut::default() }
    }
}

pub(super) fn loudness(volume: f64, own: u32) -> f64 {
    volume * f64::from(own.min(100)) / 100.0
}

impl SceneSpec {
    fn media(&self) -> Vec<(String, usize, Option<Reel>)> {
        let files = self.extra.iter().map(|(k, i)| (k.clone(), Arc::as_ptr(i) as usize, None));
        files.chain(self.clips.iter().map(|(k, reel)| (k.clone(), 0, Some(reel.clone())))).collect()
    }

    pub(super) fn reels(&self) -> impl Iterator<Item = (&str, &Reel, u32)> {
        let replaced =
            |id: &String| self.clips.iter().any(|(k, _)| k == id) || self.extra.iter().any(|(k, _)| k == id);
        let own =
            self.pack.videos.iter().filter(move |(id, _)| !replaced(id)).map(|(id, r)| (id.as_str(), r, 100));
        own.chain(self.clips.iter().map(|(id, reel)| (id.as_str(), reel, 0)))
    }
}

#[derive(Clone)]
pub struct Spec {
    pub content: Content,
    pub fit: Fit,
    pub background: [u8; 3],
    pub transition: Option<Duration>,
    pub refresh: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub transition: Duration,
    pub max_fps: u32,
    pub volume: f64,
    pub muted: bool,
    pub software_parallax: bool,
    pub flash_guard: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OutputStatus {
    pub kind: &'static str,
    pub playing: bool,
    pub error: Option<String>,
    pub parallax: bool,
    pub frames: u64,
    pub fps: u32,
    pub sound: &'static str,
    pub flashes: &'static str,
    pub particles: &'static str,
    pub left_out: Vec<String>,
}

enum Command {
    Configure(String, (u32, u32), Spec),
    Remove(String),
    Attach(String, Window),
    Frame(String),
    Repaint(String),
    Backdrop(String),
    SetActive(String, bool, bool),
    Settings(Settings),
    Calm(bool),
    Power(bool),
    Saver(bool),
    Locked(bool),
    Press(String, String),
    Retry,
    Wake,
    Quit,
}

pub struct Compositor {
    tx: mpsc::Sender<Command>,
    status: Arc<Mutex<BTreeMap<String, OutputStatus>>>,
    limits: TargetLimits,
    thread: Option<JoinHandle<()>>,
}

impl Compositor {
    pub fn start(
        renderer: &'static Renderer,
        exporter: Arc<Exporter>,
        settings: Settings,
        on_status: impl Fn(Vec<String>) + Send + 'static,
    ) -> std::io::Result<Self> {
        renderer.on_device_lost(|reason| stops::lost(&format!("the GPU device was lost ({reason})")));
        let limits = renderer.limits();
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(BTreeMap::new()));
        let mut state = State::new(renderer, exporter, settings, tx.clone(), status.clone());
        state.on_status = Box::new(on_status);
        let thread = std::thread::Builder::new().name("wprs-render".into()).spawn(move || state.run(&rx))?;
        Ok(Self { tx, status, limits, thread: Some(thread) })
    }

    pub fn waker(&self) -> impl Fn() + Send + Sync + 'static {
        let tx = self.tx.clone();
        move || drop(tx.send(Command::Wake))
    }

    pub fn framer(&self) -> impl Fn(&str) + Send + Sync + 'static {
        self.relay(Command::Frame)
    }

    pub fn repainter(&self) -> impl Fn(&str) + Send + Sync + 'static {
        self.relay(Command::Repaint)
    }

    pub fn backdropper(&self) -> impl Fn(&str) + Send + Sync + 'static {
        self.relay(Command::Backdrop)
    }

    fn relay(&self, command: fn(String) -> Command) -> impl Fn(&str) + Send + Sync + 'static {
        let tx = self.tx.clone();
        move |output| drop(tx.send(command(output.into())))
    }

    pub fn check_target(&self, width: u32, height: u32) -> Result<(), RenderError> {
        self.limits.check(width, height)
    }

    pub fn max_edge(&self) -> u32 {
        self.limits.max_edge
    }

    pub fn configure(&self, output: &str, size: (u32, u32), spec: Spec) {
        match self.check_target(size.0, size.1) {
            Ok(()) => self.send(Command::Configure(output.into(), size, spec)),
            Err(e) => tracing::warn!(output, "cannot render this output: {e}"),
        }
    }

    pub fn remove(&self, output: &str) {
        self.send(Command::Remove(output.into()))
    }

    pub fn attach(&self, output: &str, window: Window) {
        self.send(Command::Attach(output.into(), window))
    }

    pub fn set_active(&self, output: &str, active: bool, covered: bool) {
        self.send(Command::SetActive(output.into(), active, covered))
    }

    pub fn set_settings(&self, settings: Settings) {
        self.send(Command::Settings(settings))
    }

    pub fn calm(&self, calm: bool) {
        self.send(Command::Calm(calm))
    }

    pub fn set_power(&self, on_battery: bool) {
        self.send(Command::Power(on_battery))
    }

    pub fn set_saver(&self, saver: bool) {
        self.send(Command::Saver(saver))
    }

    pub fn set_locked(&self, locked: bool) {
        self.send(Command::Locked(locked))
    }

    pub fn press(&self, output: &str, id: &str) {
        self.send(Command::Press(output.into(), id.into()))
    }

    pub fn retry_effects(&self) {
        self.send(Command::Retry)
    }

    pub fn status(&self) -> BTreeMap<String, OutputStatus> {
        lock(&self.status).clone()
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Quit);
    }

    pub fn running(&self) -> bool {
        self.thread.as_ref().is_some_and(|thread| !thread.is_finished())
    }

    fn send(&self, command: Command) {
        if self.tx.send(command).is_err() {
            fatal("the render thread stopped");
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        self.thread.take().map(JoinHandle::join);
    }
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
