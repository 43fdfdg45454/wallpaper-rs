pub(crate) mod covers;
#[cfg(test)]
mod fetched;
mod listen;
#[cfg(test)]
mod pictured;
mod pictures;
mod players;
#[cfg(test)]
mod restart;
mod simulated;
#[cfg(test)]
mod tests;
mod track;

use crate::compositor::lock;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use wprs_core::model::MediaStatus;
use wprs_scene::MediaState;

pub(crate) use covers::Cover;
pub(crate) use players::{Player, Status, chosen};
pub(crate) use track::Track;

type Wake = Arc<dyn Fn() + Send + Sync>;

static LISTENER: OnceLock<Arc<Listener>> = OnceLock::new();
static SERIAL: AtomicU64 = AtomicU64::new(1);
const RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub(crate) serial: u64,
    pub(crate) state: MediaState,
    pub(crate) read: Instant,
    pub(crate) cover: Option<Arc<Cover>>,
    pub(crate) previous: Option<Arc<Cover>>,
}

pub(crate) struct Listener {
    bus: zbus::Connection,
    retry: Duration,
    runtime: tokio::runtime::Handle,
    wake: Wake,
    changed: Wake,
    task: Mutex<Option<(u64, JoinHandle<()>)>>,
    latest: Mutex<Option<Arc<Frame>>>,
    simulated: Mutex<BTreeMap<String, Arc<Frame>>>,
    status: Mutex<MediaStatus>,
    remote: watch::Sender<bool>,
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
    let _ = LISTENER.set(Listener::new(bus, wake, changed, RETRY));
}

pub(crate) fn follow(on: bool) {
    if let Some(l) = LISTENER.get() {
        l.follow(on);
    }
}

impl Listener {
    pub(crate) fn new(bus: zbus::Connection, wake: Wake, changed: Wake, retry: Duration) -> Arc<Listener> {
        let runtime = tokio::runtime::Handle::current();
        let (task, latest, simulated, status) = Default::default();
        let remote = watch::Sender::new(false);
        Arc::new(Listener { bus, retry, runtime, wake, changed, task, latest, simulated, status, remote })
    }

    pub(crate) fn follow(self: &Arc<Self>, on: bool) {
        let mut task = lock(&self.task);
        if task.is_some() == on {
            return;
        }
        match task.take() {
            Some((_, running)) => {
                running.abort();
                *lock(&self.latest) = None;
                let remote = *self.remote.borrow();
                *lock(&self.status) = MediaStatus { remote_covers: remote, ..MediaStatus::default() };
            }
            None => {
                let id = SERIAL.fetch_add(1, Ordering::Relaxed);
                let listening = listen::listen(self.clone(), id, self.bus.clone(), self.remote.subscribe());
                *task = Some((id, self.runtime.spawn(listening)));
            }
        }
        tracing::info!(on, "media player listener");
        drop(task);
        (self.wake)();
        (self.changed)();
    }

    fn ended(&self, id: u64) {
        let mut task = lock(&self.task);
        if task.as_ref().is_none_or(|(running, _)| *running != id) {
            return;
        }
        *task = None;
        drop(task);
        (self.changed)();
    }

    fn publish(&self, frame: Option<Frame>, status: MediaStatus) {
        let remote = *self.remote.borrow();
        let status = MediaStatus { remote_covers: remote, using: lock(&self.status).using.clone(), ..status };
        let changed = std::mem::replace(&mut *lock(&self.status), status.clone()) != status;
        *lock(&self.latest) = frame.map(Arc::new);
        (self.wake)();
        if changed {
            (self.changed)();
        }
    }
}

pub(crate) fn remote_covers(on: bool) {
    let Some(l) = LISTENER.get() else { return };
    if l.remote.send_if_modified(|was| std::mem::replace(was, on) != on) {
        lock(&l.status).remote_covers = on;
        (l.changed)();
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
        Some(frame) => {
            let next = Arc::new(simulated::continued(simulated.get(output).map(|f| &**f), (*frame).clone()));
            simulated.insert(output.to_owned(), next).is_none()
        }
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

pub(crate) fn using(outputs: std::collections::BTreeSet<String>) {
    let Some(l) = LISTENER.get() else { return };
    let changed = std::mem::replace(&mut lock(&l.status).using, outputs.clone()) != outputs;
    if changed {
        (l.changed)();
    }
}
