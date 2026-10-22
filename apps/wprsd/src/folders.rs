use crate::compositor::lock;
use crate::daemon::Daemon;
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

const MAX_WATCHES: usize = 8192;
const SPACING: Duration = Duration::from_secs(5);

struct Watch {
    source: String,
    dir: PathBuf,
    recursive: bool,
}

pub struct Folders {
    fd: Option<OwnedFd>,
    watches: Mutex<HashMap<i32, Watch>>,
    tx: mpsc::UnboundedSender<String>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<String>>>,
}

fn flags() -> WatchFlags {
    WatchFlags::CREATE
        | WatchFlags::CLOSE_WRITE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::DELETE_SELF
        | WatchFlags::ONLYDIR
}

fn visible(name: &OsStr) -> bool {
    !name.as_bytes().starts_with(b".")
}

impl Folders {
    pub fn new() -> Self {
        let fd =
            inotify::init(CreateFlags::CLOEXEC).inspect_err(|e| tracing::info!("cannot watch folders: {e}"));
        let (tx, rx) = mpsc::unbounded_channel();
        Self { fd: fd.ok(), watches: Mutex::default(), tx, rx: Mutex::new(Some(rx)) }
    }

    fn add(&self, watches: &mut HashMap<i32, Watch>, source: &str, dir: &Path, recursive: bool) {
        let mut dirs = vec![dir.to_owned()];
        while let (Some(dir), Some(fd)) = (dirs.pop(), &self.fd) {
            if watches.len() >= MAX_WATCHES {
                return;
            }
            if let Ok(wd) = inotify::add_watch(fd, &dir, flags()) {
                let entries = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter(|_| recursive);
                let subdirs =
                    entries.filter(|e| visible(&e.file_name()) && e.file_type().is_ok_and(|t| t.is_dir()));
                dirs.extend(subdirs.map(|e| e.path()));
                watches.insert(wd, Watch { source: source.into(), dir, recursive });
            }
        }
    }
}

impl Daemon {
    pub fn watch_sources(&self) {
        let Some(fd) = &self.folders.fd else { return };
        let mut watches = lock(&self.folders.watches);
        for (wd, _) in watches.drain() {
            let _ = inotify::remove_watch(fd, wd);
        }
        if !self.state().config.library.watch {
            return;
        }
        let sources = self.store.read(|lib| lib.sources()).unwrap_or_default();
        for source in sources.into_iter().filter(|s| self.provider_of(&s.path).is_none()) {
            self.folders.add(&mut watches, &source.uid, &source.path, source.recursive);
        }
    }
}

pub fn start(daemon: &Arc<Daemon>) {
    let (Some(fd), Some(rx)) = (daemon.folders.fd.as_ref(), lock(&daemon.folders.rx).take()) else { return };
    let Ok(fd) = fd.try_clone() else { return };
    let weak = Arc::downgrade(daemon);
    let reader = std::thread::Builder::new().name("wprs-folders".into()).spawn(move || read(&fd, &weak));
    reader.map(drop).unwrap_or_else(|e| tracing::warn!("cannot watch folders: {e}"));
    tokio::spawn(debounce(Arc::downgrade(daemon), rx));
    let daemon = daemon.clone();
    tokio::task::spawn_blocking(move || daemon.watch_sources());
}

fn read(fd: &OwnedFd, daemon: &Weak<Daemon>) {
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut reader = inotify::Reader::new(fd, &mut buffer);
    loop {
        let event = match reader.next() {
            Ok(event) => event,
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
        };
        let (events, name) = (event.events(), event.file_name().map(|n| OsStr::from_bytes(n.to_bytes())));
        let Some(daemon) = daemon.upgrade() else { return };
        let folders = &daemon.folders;
        let mut watches = lock(&folders.watches);
        let Some(watch) = watches.get(&event.wd()) else { continue };
        let file_created = events.contains(ReadFlags::CREATE) && !events.contains(ReadFlags::ISDIR);
        if file_created || name.is_some_and(|n| !visible(n)) {
            continue;
        }
        let (source, dir, recursive) = (watch.source.clone(), watch.dir.clone(), watch.recursive);
        if let Some(name) = name.filter(|_| events.contains(ReadFlags::ISDIR) && recursive) {
            folders.add(&mut watches, &source, &dir.join(name), true);
        }
        drop(folders.tx.send(source));
    }
}

async fn debounce(daemon: Weak<Daemon>, mut rx: mpsc::UnboundedReceiver<String>) {
    let (mut last, mut pending) = (HashMap::<String, Instant>::new(), HashMap::<String, Instant>::new());
    loop {
        let next = pending.values().min().copied();
        tokio::select! {
            source = rx.recv() => {
                let Some(source) = source else { return };
                let due = last.get(&source).map_or_else(Instant::now, |at| *at + SPACING);
                pending.entry(source).or_insert(due);
            }
            () = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                let due: Vec<String> = pending.iter().filter(|(_, at)| **at <= now).map(|(s, _)| s.clone()).collect();
                let Some(daemon) = daemon.upgrade() else { return };
                for source in due {
                    pending.remove(&source);
                    tracing::debug!(source, "folder changed");
                    daemon.scan_job(&source);
                    last.insert(source, now);
                }
            }
        }
    }
}
