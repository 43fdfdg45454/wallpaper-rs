use super::SPACING;
use crate::compositor::lock;
use crate::daemon::Daemon;
use rustix::fs::inotify;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, Weak};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};

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

pub(super) fn read(fd: &OwnedFd, daemon: &Weak<Daemon>) {
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
