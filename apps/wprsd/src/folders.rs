use crate::compositor::lock;
use crate::daemon::Daemon;
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::Duration;

mod watch;

pub use watch::start;

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

#[cfg(test)]
mod tests;
