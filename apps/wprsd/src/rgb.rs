use crate::compositor::lock;
use crate::daemon::Daemon;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use wprs_core::rgb::Rgb;
use wprs_rgb::{Device, Strip};

mod worker;

const EVERY: Duration = Duration::from_millis(33);
const KEPT: usize = 16;

pub(crate) enum Ask {
    Settings(Rgb),
    Frame(String, Strip),
    Test(Option<u32>, Sender<Result<(), String>>),
    Connect(Sender<Result<(), String>>),
}

#[derive(Default)]
pub(crate) struct Status {
    pub(crate) rgb: Rgb,
    pub(crate) connected: bool,
    pub(crate) error: Option<String>,
    pub(crate) devices: Vec<Device>,
    pub(crate) source: Option<String>,
    pub(crate) frames: u64,
    pub(crate) testing: bool,
}

struct Lights {
    asks: Sender<Ask>,
    status: Arc<Mutex<Status>>,
    strips: Mutex<BTreeMap<String, (Instant, Strip)>>,
    sent: Mutex<Option<Instant>>,
}

static LIGHTS: OnceLock<Lights> = OnceLock::new();

fn lights() -> &'static Lights {
    LIGHTS.get_or_init(|| {
        let (asks, taken) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        worker::spawn(taken, status.clone());
        Lights { asks, status, strips: Mutex::default(), sent: Mutex::default() }
    })
}

pub(crate) fn follow(daemon: &Daemon) {
    let wanted = daemon.state().config.rgb.clone();
    if !wanted.enabled && LIGHTS.get().is_none() {
        return;
    }
    let l = lights();
    if lock(&l.status).rgb != wanted {
        lock(&l.status).rgb = wanted.clone();
        l.asks.send(Ask::Settings(wanted)).ok();
        daemon.compositor.waker()();
    }
}

pub(crate) fn enabled() -> bool {
    LIGHTS.get().is_some_and(|l| lock(&l.status).rgb.enabled)
}

pub(crate) fn frame(source: &str, strip: Strip, send: bool) {
    let l = lights();
    let now = Instant::now();
    let mut strips = lock(&l.strips);
    strips.insert(source.into(), (now, strip.clone()));
    if strips.len() > KEPT
        && let Some(oldest) = strips.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone())
    {
        strips.remove(&oldest);
    }
    drop(strips);
    let mut sent = lock(&l.sent);
    if send && enabled() && sent.is_none_or(|at| now.duration_since(at) >= EVERY) {
        *sent = Some(now);
        l.asks.send(Ask::Frame(source.into(), strip)).ok();
    }
}

pub(crate) fn forget(source: &str) {
    if let Some(l) = LIGHTS.get() {
        lock(&l.strips).remove(source);
    }
}

pub(crate) fn strip(source: &str) -> Option<Strip> {
    LIGHTS.get().and_then(|l| lock(&l.strips).get(source).map(|(_, s)| s.clone()))
}

fn asked(make: impl FnOnce(Sender<Result<(), String>>) -> Ask) -> Result<(), String> {
    let (tx, rx) = channel();
    lights().asks.send(make(tx)).map_err(|_| "the RGB lighting stopped".to_string())?;
    rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "the OpenRGB server did not answer".to_string())?
}

pub(crate) fn connect() -> Result<(), String> {
    asked(Ask::Connect)
}

pub(crate) fn test(device: Option<u32>) -> Result<(), String> {
    asked(|tx| Ask::Test(device, tx))
}

pub(crate) fn status(rgb: &Rgb) -> Value {
    let Some(l) = LIGHTS.get() else {
        return json!({ "enabled": rgb.enabled, "server": rgb.server, "local": rgb.local(), "connected": false,
            "error": null, "devices": [], "source": null, "frames": 0, "testing": false, "strips": [] });
    };
    let s = lock(&l.status);
    let devices: Vec<Value> = s
        .devices
        .iter()
        .map(|d| {
            let zones: Vec<Value> = d
                .zones
                .iter()
                .map(|z| {
                    json!({ "name": z.name, "leds": z.leds,
                "matrix": z.matrix.as_ref().map(|(w, h, _)| [w, h]) })
                })
                .collect();
            json!({ "index": d.index, "name": d.name, "kind": d.kind, "leds": d.leds, "zones": zones })
        })
        .collect();
    json!({ "enabled": s.rgb.enabled, "server": s.rgb.server, "local": s.rgb.local(), "connected": s.connected,
        "error": s.error, "devices": devices, "source": s.source, "frames": s.frames, "testing": s.testing,
        "strips": lock(&l.strips).keys().collect::<Vec<_>>() })
}
