mod feed;
mod frames;
mod iio;
mod motion;
mod output;
mod parallax;
mod players;
mod power;
mod scene;
mod slot;
mod state;
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
use wprs_wayland::Window;

#[derive(Clone)]
pub enum Content {
    Color,
    Image(Picture),
    Video(PathBuf),
    Scene(SceneSpec),
}

#[derive(Clone)]
pub struct SceneSpec {
    pub pack: Arc<ScenePack>,
    pub values: Values,
    pub extra: Vec<(String, Arc<RgbaImage>)>,
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
            (Content::Video(a), Content::Video(b)) => a == b,
            (Content::Scene(a), Content::Scene(b)) => a.same_source(b),
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
            Content::Scene(s) => Some(s.values.clone()),
            _ => None,
        };
        self.same_source(other) && look(self) == look(other) && values(self) == values(other)
    }

    fn video(&self) -> Option<&PathBuf> {
        match self {
            Content::Video(path) => Some(path),
            _ => None,
        }
    }

    fn videos(&self) -> Vec<&PathBuf> {
        match self {
            Content::Video(path) => vec![path],
            Content::Scene(s) => s.pack.videos.iter().map(|(_, path)| path).collect(),
            _ => Vec::new(),
        }
    }
}

impl SceneSpec {
    fn same_source(&self, other: &SceneSpec) -> bool {
        let files = |s: &SceneSpec| s.extra.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        Arc::ptr_eq(&self.pack, &other.pack) && files(self) == files(other)
    }
}

#[derive(Clone)]
pub struct Spec {
    pub content: Content,
    pub fit: Fit,
    pub background: [u8; 3],
    pub transition: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub transition: Duration,
    pub max_fps: u32,
    pub volume: f64,
    pub muted: bool,
    pub software_parallax: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OutputStatus {
    pub kind: &'static str,
    pub playing: bool,
    pub error: Option<String>,
    pub parallax: bool,
    pub frames: u64,
}

enum Command {
    Configure(String, (u32, u32), Spec),
    Remove(String),
    Attach(String, Window),
    Frame(String),
    SetActive(String, bool),
    Settings(Settings),
    Calm(bool),
    Power(bool),
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
        renderer: Renderer,
        exporter: Arc<Exporter>,
        settings: Settings,
        on_status: impl Fn(Vec<String>) + Send + 'static,
    ) -> std::io::Result<Self> {
        renderer.on_device_lost(|reason| fatal(&format!("the GPU device was lost ({reason})")));
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
        let tx = self.tx.clone();
        move |output| drop(tx.send(Command::Frame(output.into())))
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

    pub fn set_active(&self, output: &str, active: bool) {
        self.send(Command::SetActive(output.into(), active))
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

    pub fn status(&self) -> BTreeMap<String, OutputStatus> {
        lock(&self.status).clone()
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
