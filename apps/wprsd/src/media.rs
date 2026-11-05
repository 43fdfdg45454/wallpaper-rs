pub(crate) mod covers;
mod listen;
#[cfg(test)]
mod pictured;
mod pictures;
mod players;
#[cfg(test)]
mod tests;
mod track;

use crate::compositor::lock;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio::task::JoinHandle;
use wprs_core::model::MediaStatus;
use wprs_scene::MediaState;

pub(crate) use covers::Cover;
pub(crate) use players::{Player, Status, chosen};
pub(crate) use track::Track;

type Wake = Arc<dyn Fn() + Send + Sync>;

static LISTENER: OnceLock<Listener> = OnceLock::new();
static SERIAL: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub(crate) serial: u64,
    pub(crate) state: MediaState,
    pub(crate) read: Instant,
    pub(crate) cover: Option<Arc<Cover>>,
    pub(crate) previous: Option<Arc<Cover>>,
}

struct Listener {
    bus: zbus::Connection,
    runtime: tokio::runtime::Handle,
    wake: Wake,
    changed: Wake,
    task: Mutex<Option<JoinHandle<()>>>,
    latest: Mutex<Option<Arc<Frame>>>,
    simulated: Mutex<BTreeMap<String, Arc<Frame>>>,
    status: Mutex<MediaStatus>,
    remote: AtomicBool,
}

impl Frame {
    pub(crate) fn new(state: MediaState) -> Frame {
        let (serial, read) = (SERIAL.fetch_add(1, Ordering::Relaxed), Instant::now());
        Frame { serial, state, read, cover: None, previous: None }
    }

    pub(crate) fn at(&self, now: Instant) -> MediaState {
        let mut state = self.state.clone();
        if state.playing {
            let spent = now.saturating_duration_since(self.read).as_secs_f32();
            let moved = state.position + spent * state.rate;
            state.position = if state.duration > 0.0 { moved.min(state.duration) } else { moved };
        }
        state
    }
}

pub(crate) fn start(bus: zbus::Connection, wake: Wake, changed: Wake) {
    let runtime = tokio::runtime::Handle::current();
    let (task, latest, simulated, status, remote) = Default::default();
    let made = Listener { bus, runtime, wake, changed, task, latest, simulated, status, remote };
    let _ = LISTENER.set(made);
}

pub(crate) fn follow(on: bool) {
    let Some(l) = LISTENER.get() else { return };
    let mut task = lock(&l.task);
    if task.is_some() == on {
        return;
    }
    match task.take() {
        Some(running) => {
            running.abort();
            *lock(&l.latest) = None;
            let remote = l.remote.load(Ordering::Relaxed);
            *lock(&l.status) = MediaStatus { remote_covers: remote, ..MediaStatus::default() };
        }
        None => *task = Some(l.runtime.spawn(listen::listen(l.bus.clone()))),
    }
    tracing::info!(on, "media player listener");
    drop(task);
    (l.wake)();
    (l.changed)();
}

pub(crate) fn remote_on() -> bool {
    LISTENER.get().is_some_and(|l| l.remote.load(Ordering::Relaxed))
}

pub(crate) fn remote_covers(on: bool) {
    if let Some(l) = LISTENER.get() {
        l.remote.store(on, Ordering::Relaxed);
        lock(&l.status).remote_covers = on;
    }
}

pub(crate) fn frame(output: &str, allowed: bool) -> Option<Arc<Frame>> {
    let l = LISTENER.get()?;
    if let Some(simulated) = lock(&l.simulated).get(output) {
        return Some(simulated.clone());
    }
    allowed.then(|| lock(&l.latest).clone()).flatten()
}

pub(crate) fn of(output: &str) -> Option<Arc<Frame>> {
    frame(output, crate::inputs::allows(output, wprs_core::model::Input::Media))
}

pub(crate) fn simulate(output: &str, frame: Option<Arc<Frame>>) {
    let Some(l) = LISTENER.get() else { return };
    let mut simulated = lock(&l.simulated);
    let changed = match frame {
        Some(frame) => simulated.insert(output.to_owned(), frame).is_none(),
        None => simulated.remove(output).is_some(),
    };
    drop(simulated);
    (l.wake)();
    if changed {
        crate::inputs::poke();
    }
}

pub(crate) fn simulated(output: &str) -> bool {
    LISTENER.get().is_some_and(|l| lock(&l.simulated).contains_key(output))
}

pub(crate) fn status() -> MediaStatus {
    LISTENER.get().map(|l| lock(&l.status).clone()).unwrap_or_default()
}

fn publish(frame: Option<Frame>, status: MediaStatus) {
    let Some(l) = LISTENER.get() else { return };
    let remote = l.remote.load(Ordering::Relaxed);
    let status = MediaStatus { remote_covers: remote, using: lock(&l.status).using.clone(), ..status };
    let changed = std::mem::replace(&mut *lock(&l.status), status.clone()) != status;
    *lock(&l.latest) = frame.map(Arc::new);
    (l.wake)();
    if changed {
        (l.changed)();
    }
}

pub(crate) fn using(outputs: std::collections::BTreeSet<String>) {
    let Some(l) = LISTENER.get() else { return };
    let changed = std::mem::replace(&mut lock(&l.status).using, outputs.clone()) != outputs;
    if changed {
        (l.changed)();
    }
}
