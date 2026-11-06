use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::fs::inotify::{self, CreateFlags};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(in crate::compositor) type Wake = Arc<dyn Fn() + Send + Sync>;

pub(super) struct Watch {
    pub(super) dirs: Vec<PathBuf>,
    pub(super) changed: Arc<AtomicBool>,
    stop: Arc<OwnedFd>,
}

pub(super) fn dirs(top: &Path, depth: u8) -> Vec<PathBuf> {
    let (mut out, mut stack) = (Vec::new(), vec![(top.to_owned(), 0u8)]);
    while let Some((dir, level)) = stack.pop().filter(|_| out.len() < 256) {
        let below = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter(|_| level < depth);
        let below = below.filter(|e| !e.file_name().to_string_lossy().starts_with('.'));
        stack
            .extend(below.filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| (e.path(), level + 1)));
        out.push(dir);
    }
    out
}

impl Watch {
    pub(super) fn new(dirs: Vec<PathBuf>, wake: Wake) -> Option<Watch> {
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).ok()?;
        let marked =
            dirs.iter().filter(|d| inotify::add_watch(&fd, d.as_path(), crate::folders::flags()).is_ok());
        if marked.count() == 0 {
            return None;
        }
        let stop = Arc::new(eventfd(0, EventfdFlags::CLOEXEC).ok()?);
        let changed = Arc::new(AtomicBool::new(true));
        let (seen, ended) = (Arc::clone(&changed), Arc::clone(&stop));
        let reader = std::thread::Builder::new().name("wprs-listing".into());
        reader.spawn(move || read(&fd, &ended, &seen, &wake)).ok()?;
        Some(Watch { dirs, changed, stop })
    }
}

fn read(fd: &OwnedFd, stop: &OwnedFd, changed: &AtomicBool, wake: &Wake) {
    let mut buffer = [MaybeUninit::uninit(); 4096];
    loop {
        let mut fds = [PollFd::new(fd, PollFlags::IN), PollFd::new(stop, PollFlags::IN)];
        match poll(&mut fds, None) {
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
            Ok(_) if !fds[1].revents().is_empty() => return,
            Ok(_) => {}
        }
        let mut reader = inotify::Reader::new(fd, &mut buffer);
        let mut any = false;
        while reader.next().is_ok() {
            any = true;
        }
        if any && !changed.swap(true, Ordering::AcqRel) {
            wake();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = rustix::io::write(&*self.stop, &1u64.to_ne_bytes());
    }
}
