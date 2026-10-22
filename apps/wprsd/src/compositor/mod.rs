mod frames;
mod output;
mod players;
mod slot;
mod state;
mod surface;

use crate::fatal;
use image::RgbaImage;
use state::State;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use wprs_core::{Edit, Fit, Place};
use wprs_pipewire::Exporter;
use wprs_render::{RenderError, Renderer, TargetLimits};
use wprs_wayland::Window;

#[derive(Clone)]
pub enum Content {
    Color,
    Image(Picture),
    Video(PathBuf),
}

#[derive(Clone)]
pub struct Picture {
    pub file: PathBuf,
    pub modified: Option<SystemTime>,
    pub pixels: Option<Arc<RgbaImage>>,
    pub edit: Option<Arc<Edit>>,
    pub place: Place,
}

impl Content {
    fn same_source(&self, other: &Content) -> bool {
        match (self, other) {
            (Content::Color, Content::Color) => true,
            (Content::Image(a), Content::Image(b)) => (&a.file, a.modified) == (&b.file, b.modified),
            (Content::Video(a), Content::Video(b)) => a == b,
            _ => false,
        }
    }

    fn same_look(&self, other: &Content) -> bool {
        let look = |c: &Content| match c {
            Content::Image(p) => Some((p.edit.clone(), p.place.clone())),
            _ => None,
        };
        self.same_source(other) && look(self) == look(other)
    }

    fn video(&self) -> Option<&PathBuf> {
        match self {
            Content::Video(path) => Some(path),
            _ => None,
        }
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
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OutputStatus {
    pub kind: &'static str,
    pub playing: bool,
    pub error: Option<String>,
}

enum Command {
    Configure(String, (u32, u32), Spec),
    Remove(String),
    Attach(String, Window),
    Frame(String),
    SetActive(String, bool),
    Settings(Settings),
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

    pub fn status(&self) -> BTreeMap<String, OutputStatus> {
        lock(&self.status).clone()
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
