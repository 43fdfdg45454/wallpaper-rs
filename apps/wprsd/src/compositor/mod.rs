mod frames;
mod output;
mod players;
mod slot;
mod state;

use image::RgbaImage;
use state::State;
use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;
use wprs_core::Fit;
use wprs_pipewire::Exporter;
use wprs_render::{RenderError, Renderer, TargetLimits};

#[derive(Clone)]
pub enum Content {
    Color,
    Image(Arc<RgbaImage>),
    Video(PathBuf),
}

impl Content {
    fn same_source(&self, other: &Content) -> bool {
        match (self, other) {
            (Content::Color, Content::Color) => true,
            (Content::Image(a), Content::Image(b)) => Arc::ptr_eq(a, b),
            (Content::Video(a), Content::Video(b)) => a == b,
            _ => false,
        }
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
    pub fn start(renderer: Renderer, exporter: Arc<Exporter>, settings: Settings) -> std::io::Result<Self> {
        renderer.on_device_lost(|reason| fatal(&format!("the GPU device was lost ({reason})")));
        let limits = renderer.limits();
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(BTreeMap::new()));
        let mut state = State::new(renderer, exporter, settings, tx.clone(), status.clone());
        let run = move || {
            if std::panic::catch_unwind(AssertUnwindSafe(|| state.run(&rx))).is_err() {
                fatal("the render thread panicked");
            }
        };
        let thread = std::thread::Builder::new().name("wprs-render".into()).spawn(run)?;
        Ok(Self { tx, status, limits, thread: Some(thread) })
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
            tracing::error!("the render thread is not running");
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

fn fatal(reason: &str) -> ! {
    tracing::error!("{reason}; exiting so that the service manager restarts the daemon");
    std::process::exit(70)
}
