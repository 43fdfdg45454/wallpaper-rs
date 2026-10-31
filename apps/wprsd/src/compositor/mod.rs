mod feed;
mod frames;
mod guard;
mod iio;
mod motion;
mod output;
mod pages;
mod parallax;
mod pictures;
mod players;
mod power;
mod scene;
mod scripts;
mod slot;
mod speakers;
mod spec;
mod state;
pub(crate) mod stops;
mod surface;

use crate::fatal;
pub use spec::*;
use state::State;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;
use wprs_pipewire::Exporter;
use wprs_render::{RenderError, Renderer, TargetLimits};
use wprs_wayland::Window;

enum Command {
    Configure(String, (u32, u32), Spec),
    Remove(String),
    Attach(String, Window),
    Frame(String),
    Repaint(String),
    Region(String),
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

    pub fn regioner(&self) -> impl Fn(&str) + Send + Sync + 'static {
        self.relay(Command::Region)
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
