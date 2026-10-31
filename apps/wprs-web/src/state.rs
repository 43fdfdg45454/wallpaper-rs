use crate::ffi::Ptr;
use crate::ring::Ring;
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use wprs_web::{Allowed, Folder, FromWeb, Link, Settings, Start};

pub struct Engine {
    pub link: Link,
    pub start: Start,
    pub context: Ptr,
    pub session: Ptr,
    pub entries: HashSet<String>,
    pub preload: Option<String>,
}

pub struct View {
    pub id: u32,
    pub web: Ptr,
    pub wpe: Ptr,
    pub manager: Ptr,
    pub fps: Cell<u32>,
    pub last: Cell<Option<Instant>>,
    pub visible: Cell<bool>,
    pub hidden_at: Cell<Option<Instant>>,
    pub stopped: Cell<bool>,
    pub crashes: Cell<u32>,
    pub known: RefCell<Vec<(Ptr, u32)>>,
    pub held: RefCell<HashMap<u32, Ptr>>,
    pub ring: RefCell<Option<Ring>>,
    pub loaded: Cell<bool>,
    pub answered: Cell<Instant>,
}

#[derive(Default)]
pub struct Live {
    pub settings: Settings,
    pub allowed: Allowed,
    pub properties: serde_json::Map<String, serde_json::Value>,
    pub views: BTreeMap<u32, serde_json::Map<String, serde_json::Value>>,
    pub folders: BTreeMap<String, Folder>,
    pub files: HashMap<String, PathBuf>,
    pub audio: bool,
    pub copy: bool,
    pub said: u32,
}

thread_local! {
    static ENGINE: OnceCell<&'static Engine> = const { OnceCell::new() };
    pub static VIEWS: RefCell<BTreeMap<u32, Rc<View>>> = const { RefCell::new(BTreeMap::new()) };
    pub static LIVE: RefCell<Live> = RefCell::new(Live::default());
    pub static FILTER: Cell<Ptr> = const { Cell::new(std::ptr::null_mut()) };
    pub static WAITING: RefCell<Vec<wprs_web::ToWeb>> = const { RefCell::new(Vec::new()) };
}

pub fn set_engine(engine: Engine) {
    let leaked: &'static Engine = Box::leak(Box::new(engine));
    ENGINE.with(|e| {
        let _ = e.set(leaked);
    });
}

pub fn engine() -> Option<&'static Engine> {
    ENGINE.with(|e| e.get().copied())
}

pub fn send(message: &FromWeb) {
    if let Some(engine) = engine() {
        say(&engine.link, message, &[]);
    }
}

pub fn say(link: &Link, message: &FromWeb, fds: &[std::os::fd::BorrowedFd<'_>]) {
    if let Err(e) = link.send(message, fds) {
        eprintln!("wprs-web: cannot reach the daemon: {e}");
    }
}

pub fn view(id: u32) -> Option<Rc<View>> {
    VIEWS.with(|v| v.borrow().get(&id).cloned())
}

pub fn by_wpe(wpe: Ptr) -> Option<Rc<View>> {
    VIEWS.with(|v| v.borrow().values().find(|v| v.wpe == wpe).cloned())
}

pub fn by_web(web: Ptr) -> Option<Rc<View>> {
    VIEWS.with(|v| v.borrow().values().find(|v| v.web == web).cloned())
}

pub fn all() -> Vec<Rc<View>> {
    VIEWS.with(|v| v.borrow().values().cloned().collect())
}

pub fn live<T>(f: impl FnOnce(&mut Live) -> T) -> T {
    LIVE.with(|l| f(&mut l.borrow_mut()))
}
