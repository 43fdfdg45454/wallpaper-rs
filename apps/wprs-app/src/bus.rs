use crate::args::{FAILED, invalid};
use crate::calls::dispatch;
use cxx_qt::{CxxQtThread, Threading};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch};
use wprs_client::{Client, Error};
use wprs_core::model::Header;

pub const CHECK: &str = "WPRS_APP_CHECK";
const UNSAFE: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'%').add(b'?');

#[derive(Clone)]
pub enum Daemon {
    Bus(Client),
    Fixtures,
}

#[derive(Clone, Debug)]
pub enum Event {
    Link(Result<(), String>),
    Changed(String, Vec<String>, u64),
    Progress(u64, u64, u64),
    Finished(u64, String, String, String),
    Thumb(String, u32, String),
    Preview(String, String),
    Activate(String, Option<String>),
    Error(String),
}

type Listener = Box<dyn Fn(Event) -> bool + Send>;

struct Bus {
    rt: Runtime,
    listeners: Mutex<Vec<Listener>>,
    farewells: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    link: watch::Sender<Option<Result<Daemon, String>>>,
    seen: Mutex<HashMap<String, u64>>,
    page: Mutex<Option<String>>,
}

static BUS: LazyLock<Bus> = LazyLock::new(|| Bus {
    rt: tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap(),
    listeners: Mutex::default(),
    farewells: Mutex::default(),
    link: watch::channel(None).0,
    seen: Mutex::default(),
    page: Mutex::default(),
});

pub fn checking() -> bool {
    std::env::var_os(CHECK).is_some()
}

pub fn block_on<T>(task: impl Future<Output = T>) -> T {
    BUS.rt.block_on(task)
}

pub fn spawn(task: impl Future<Output = ()> + Send + 'static) {
    BUS.rt.spawn(task);
}

pub fn farewell(task: impl Future<Output = ()> + Send + 'static) {
    let mut farewells = BUS.farewells.lock().unwrap();
    farewells.retain(|task| !task.is_finished());
    farewells.push(BUS.rt.spawn(task));
}

pub fn part(wait: std::time::Duration) {
    let tasks = std::mem::take(&mut *BUS.farewells.lock().unwrap());
    let all = async {
        for task in tasks {
            task.await.ok();
        }
    };
    BUS.rt.block_on(async move { tokio::time::timeout(wait, all).await.ok() });
}

pub fn publish(event: Event) {
    tell(&mut BUS.listeners.lock().unwrap(), event);
}

fn tell(listeners: &mut Vec<Listener>, event: Event) {
    listeners.retain(|send| send(event.clone()));
}

pub fn request(page: &str, token: Option<String>) {
    *BUS.page.lock().unwrap() = Some(page.to_owned());
    publish(Event::Activate(page.to_owned(), token));
}

pub fn requested() -> Option<String> {
    BUS.page.lock().unwrap().take()
}

pub fn events() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    BUS.listeners.lock().unwrap().push(Box::new(move |event| tx.send(event).is_ok()));
    rx
}

pub fn set_link(link: Result<Daemon, String>) {
    let mut listeners = BUS.listeners.lock().unwrap();
    let event = Event::Link(link.as_ref().map(|_| ()).map_err(Clone::clone));
    BUS.link.send_replace(Some(link));
    tell(&mut listeners, event);
}

pub fn saw(topic: &str, serial: u64) {
    let mut seen = BUS.seen.lock().unwrap();
    let last = seen.entry(topic.to_owned()).or_default();
    *last = (*last).max(serial);
}

pub async fn daemon() -> Result<Daemon, Error> {
    let mut link = BUS.link.subscribe();
    let link = link.wait_for(Option::is_some).await.map_err(|e| invalid(e.to_string()))?;
    let failed = |message| Error { name: FAILED.into(), message };
    link.clone().unwrap_or(Err(String::new())).map_err(failed)
}

pub async fn call(iface: &str, member: &str, args: Vec<Value>) -> Result<String, Error> {
    let iface = iface.strip_prefix("io.github.wallpaper_rs.").unwrap_or(iface);
    let daemon = daemon().await?;
    let seen = || BUS.seen.lock().unwrap().get(topic(iface, member)).copied().unwrap_or(0);
    let mut reply = dispatch(&daemon, iface, member, args.clone()).await?;
    for _ in 0..2 {
        match serde_json::from_str::<Header>(&reply) {
            Ok(header) if header.serial < seen() => {
                reply = dispatch(&daemon, iface, member, args.clone()).await?
            }
            _ => break,
        }
    }
    Ok(reply)
}

pub fn topic(iface: &str, member: &str) -> &'static str {
    match (iface, member) {
        ("Daemon1", "GetState" | "GetSceneProperties") => "state",
        ("Daemon1", "ListJobs" | "GetJob") => "jobs",
        ("Library1", _) => "library",
        ("Playlists1", _) => "playlists",
        ("Profiles1", _) => "profiles",
        ("Providers1", _) => "providers",
        ("Repos1", _) => "repos",
        ("Inputs1", _) => "inputs",
        ("Sync1", _) => "account",
        _ => "",
    }
}

pub fn url(path: &str) -> String {
    format!("file://{}", utf8_percent_encode(path, UNSAFE))
}

pub fn listen<T: Threading + 'static>(qt: CxxQtThread<T>, handle: fn(Pin<&mut T>, Event)) {
    let mut listeners = BUS.listeners.lock().unwrap();
    let link = BUS.link.borrow().as_ref().map(|link| link.as_ref().map(|_| ()).map_err(Clone::clone));
    if let Some(link) = link {
        qt.queue(move |this| handle(this, Event::Link(link))).ok();
    }
    listeners.push(Box::new(move |event| qt.queue(move |this| handle(this, event)).is_ok()));
}
