use crate::compositor::lock;
use crate::daemon::Daemon;
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
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
        | WatchFlags::MOVE_SELF
        | WatchFlags::ONLYDIR
}

fn visible(name: &OsStr) -> bool {
    !name.as_bytes().starts_with(b".")
}

fn linked(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink() || m.nlink() > 1)
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

    fn forget(&self, watches: &mut HashMap<i32, Watch>, gone: &Path) {
        let stale: Vec<i32> =
            watches.iter().filter(|(_, w)| w.dir.starts_with(gone)).map(|(wd, _)| *wd).collect();
        for wd in stale {
            watches.remove(&wd);
            self.fd.as_ref().map(|fd| inotify::remove_watch(fd, wd));
        }
    }

    fn handle(&self, wd: i32, events: ReadFlags, name: Option<&OsStr>) -> Option<String> {
        let mut watches = lock(&self.watches);
        let watch = watches.get(&wd)?;
        let (source, dir, recursive) = (watch.source.clone(), watch.dir.clone(), watch.recursive);
        if events.contains(ReadFlags::IGNORED) {
            watches.remove(&wd);
            return None;
        }
        if events.contains(ReadFlags::MOVE_SELF) {
            self.forget(&mut watches, &dir);
            return Some(source);
        }
        let Some(name) = name else { return Some(source) };
        let (path, folder) = (dir.join(name), events.contains(ReadFlags::ISDIR));
        if !visible(name) {
            return None;
        }
        if folder && events.intersects(ReadFlags::MOVED_FROM | ReadFlags::DELETE) {
            self.forget(&mut watches, &path);
        }
        if folder && recursive && events.intersects(ReadFlags::CREATE | ReadFlags::MOVED_TO) {
            self.add(&mut watches, &source, &path, true);
        }
        let written_later = events.contains(ReadFlags::CREATE) && !folder && !linked(&path);
        (!written_later).then_some(source)
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
        let name = event.file_name().map(|n| OsStr::from_bytes(n.to_bytes()));
        let Some(daemon) = daemon.upgrade() else { return };
        if let Some(source) = daemon.folders.handle(event.wd(), event.events(), name) {
            drop(daemon.folders.tx.send(source));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn drain(folders: &Folders) -> Vec<(Option<String>, Option<String>)> {
        let fd = folders.fd.as_ref().unwrap();
        rustix::io::ioctl_fionbio(fd, true).unwrap();
        let mut buffer = [MaybeUninit::uninit(); 8192];
        let mut reader = inotify::Reader::new(fd, &mut buffer);
        let mut seen = Vec::new();
        while let Ok(event) = reader.next() {
            let name = event.file_name().map(|n| OsStr::from_bytes(n.to_bytes()));
            let source = folders.handle(event.wd(), event.events(), name);
            seen.push((name.map(|n| n.to_string_lossy().into_owned()), source));
        }
        seen
    }

    fn watched(folders: &Folders) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = lock(&folders.watches).values().map(|w| w.dir.clone()).collect();
        dirs.sort();
        dirs
    }

    #[test]
    fn links_moved_and_removed_folders_keep_the_watches_right() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pictures");
        fs::create_dir_all(root.join("trip")).unwrap();
        let folders = Folders::new();
        folders.add(&mut lock(&folders.watches), "S", &root, true);
        assert_eq!(watched(&folders), [root.clone(), root.join("trip")]);
        fs::write(dir.path().join("elsewhere.png"), b"png").unwrap();
        fs::hard_link(dir.path().join("elsewhere.png"), root.join("linked.png")).unwrap();
        let writing = fs::File::create(root.join("new.png")).unwrap();
        let seen = drain(&folders);
        let source =
            |file: &str| seen.iter().find(|(name, _)| name.as_deref() == Some(file)).unwrap().1.clone();
        assert_eq!((source("linked.png"), source("new.png")), (Some("S".into()), None));
        drop(writing);
        let closed = (Some("new.png".into()), Some("S".into()));
        let started = std::time::Instant::now();
        while !drain(&folders).contains(&closed) {
            assert!(started.elapsed().as_secs() < 5, "the written file was never reported");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        fs::rename(root.join("trip"), dir.path().join("away")).unwrap();
        fs::create_dir(root.join("later")).unwrap();
        drain(&folders);
        assert_eq!(watched(&folders), [root.clone(), root.join("later")]);
        fs::remove_dir(root.join("later")).unwrap();
        fs::write(dir.path().join("away").join("gone.png"), b"png").unwrap();
        assert!(drain(&folders).iter().all(|(name, _)| name.as_deref() != Some("gone.png")));
        assert_eq!(watched(&folders), std::slice::from_ref(&root));
        fs::rename(&root, dir.path().join("moved")).unwrap();
        assert!(drain(&folders).iter().any(|(_, source)| source.as_deref() == Some("S")));
        assert!(watched(&folders).is_empty());
    }
}
