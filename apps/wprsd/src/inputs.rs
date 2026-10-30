mod control;
mod hub;
mod keys;
mod kwin;
mod parallax;
mod store;
#[cfg(test)]
mod tests;

use crate::compositor::lock;
use crate::daemon::Daemon;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::{Notify, watch};
use wprs_audio::{Audio, Capture};
use wprs_core::model::{Input, SceneInputs};
use wprs_scene::{AudioFrame, MAX_PRESSES, PointerState, Press};
use wprs_wayland::Window;

pub(crate) use keys::{Keys, draft_key};
pub(crate) use parallax::{pointer_over, relief};
pub(crate) use store::{declined_path, grants_path, load, load_bound, save};

const PACE: Duration = Duration::from_millis(16);

static HUB: OnceLock<Arc<Hub>> = OnceLock::new();

pub(crate) type Grants = BTreeMap<String, BTreeSet<Input>>;
pub(crate) type Bound = BTreeMap<String, Grants>;
type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct View {
    pub(crate) active: bool,
    pub(crate) covered: bool,
    pub(crate) rect: Option<[f64; 4]>,
    pub(crate) clicks: bool,
}

#[derive(Default)]
pub(crate) struct Control {
    pub(crate) grants: Bound,
    pub(crate) declined: Grants,
    pub(crate) shown: BTreeMap<String, SceneInputs>,
    pub(crate) parallax: BTreeSet<String>,
    pub(crate) views: BTreeMap<String, View>,
    pub(crate) followed: BTreeSet<String>,
    pub(crate) capturing: bool,
    pub(crate) blocked: bool,
    windows: BTreeMap<String, Window>,
}

#[derive(Default)]
struct Switch {
    audio: bool,
    capture: Option<Capture>,
}

pub(crate) struct Hub {
    pub(crate) control: Mutex<Control>,
    pub(crate) tracking: watch::Sender<bool>,
    switch: Mutex<Switch>,
    audio: Mutex<Audio>,
    pointers: Mutex<BTreeMap<String, PointerState>>,
    presses: Mutex<BTreeMap<String, Vec<Press>>>,
    reported: Mutex<BTreeMap<String, bool>>,
    moved: Notify,
    poke: Notify,
    wake: Wake,
    changed: Wake,
}

pub(crate) fn hub() -> Option<&'static Arc<Hub>> {
    HUB.get()
}

pub(crate) fn start(daemon: &Arc<Daemon>, connection: zbus::Connection, grants: (Bound, Grants)) {
    let weak = Arc::downgrade(daemon);
    let changed = move || weak.upgrade().into_iter().for_each(|d| d.changes.changed("inputs", vec![]));
    let wake = Arc::new(daemon.compositor.waker());
    let hub = HUB.get_or_init(|| Arc::new(Hub::new(grants, wake, Arc::new(changed))));
    tokio::spawn(follow(daemon.clone(), hub.clone()));
    tokio::spawn(pace(hub.clone()));
    tokio::spawn(kwin::follow(connection, hub.tracking.subscribe()));
}

pub(crate) async fn stop(connection: &zbus::Connection) {
    if hub().is_some_and(|hub| *hub.tracking.borrow()) {
        let _ = tokio::time::timeout(Duration::from_secs(1), kwin::kwin(connection, false)).await;
    }
}

async fn follow(daemon: Arc<Daemon>, hub: Arc<Hub>) {
    let mut serial = daemon.changes.subscribe();
    loop {
        let blocked = !daemon.state().config.playback.scene_capture;
        let (views, task) = ((daemon.views(), blocked), hub.clone());
        let _ = tokio::task::spawn_blocking(move || task.update(Some(views))).await;
        tokio::select! {
            () = hub.poke.notified() => {}
            changed = serial.changed() => if changed.is_err() { return },
        }
    }
}

async fn pace(hub: Arc<Hub>) {
    loop {
        hub.moved.notified().await;
        (hub.wake)();
        tokio::time::sleep(PACE).await;
    }
}

pub(crate) fn needs(
    output: &str,
    (scene, title, key): (&str, &str, &str),
    [audio, pointer, clicks]: [bool; 3],
) {
    let Some(hub) = hub() else { return };
    let (scene, title, key, granted) = (scene.into(), title.into(), key.into(), BTreeSet::new());
    let need = SceneInputs { scene, audio, pointer, clicks, title, key, granted };
    let mut control = lock(&hub.control);
    let changed = match audio || pointer || clicks {
        true => control.shown.insert(output.into(), need.clone()) != Some(need),
        false => control.shown.remove(output).is_some(),
    };
    drop(control);
    if changed {
        hub.poke.notify_one();
        (hub.changed)();
    }
}

pub(crate) fn poke() {
    hub().into_iter().for_each(|hub| hub.poke.notify_one());
}

pub(crate) fn kwin_script_loaded() -> bool {
    kwin::LOADED.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) fn audio() -> AudioFrame {
    hub().map_or_else(AudioFrame::default, |hub| lock(&hub.audio).frame())
}

pub(crate) fn hears(output: &str) -> bool {
    hub().is_some_and(|hub| lock(&hub.control).allows(output, Input::Audio))
}

pub(crate) fn window(output: &str, window: Option<Window>) {
    let Some(hub) = hub() else { return };
    let mut control = lock(&hub.control);
    let follow = window.clone().filter(|_| control.followed.contains(output));
    match window {
        Some(window) => drop(control.windows.insert(output.into(), window)),
        None => drop(control.windows.remove(output)),
    }
    drop(control);
    if let Some(window) = follow {
        window.track_pointer(true);
    }
}

pub(crate) fn touch(output: &str) -> (PointerState, Vec<Press>, [bool; 2]) {
    hub().map(|hub| hub.touch(output)).unwrap_or_default()
}

pub(crate) fn surface_pointer(output: &str, pos: Option<[f32; 2]>, buttons: u8) {
    if let Some(hub) = hub() {
        let state = PointerState { pos, buttons, inside: pos.is_some(), ..PointerState::default() };
        hub.store([(output.to_owned(), state)]);
    }
}

fn pressed(presses: &mut BTreeMap<String, Vec<Press>>, output: &str, state: &PointerState) {
    let list = presses.entry(output.to_owned()).or_default();
    if list.len() < MAX_PRESSES {
        list.push(Press { at: state.pos, buttons: state.buttons });
    }
}
