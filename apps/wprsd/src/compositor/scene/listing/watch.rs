use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(in crate::compositor) type Wake = Arc<dyn Fn() + Send + Sync>;

const REMOTE: [u32; 10] = [
    0x6969,
    0x517b,
    0xff53_4d42,
    0xfe53_4d42,
    0x6573_5546,
    0x0102_1997,
    0x00c3_6400,
    0x5346_414f,
    0x7375_7245,
    0x564c,
];

pub(in crate::compositor) struct Watch {
    pub(super) dirs: Vec<PathBuf>,
    pub(super) changed: Arc<AtomicBool>,
    dead: Arc<AtomicBool>,
    missing: bool,
    remote: bool,
    stop: Arc<OwnedFd>,
}

pub(super) fn dirs(top: &Path, depth: u8) -> Vec<PathBuf> {
    let (mut out, mut stack) = (Vec::new(), vec![(top.to_owned(), 0u8)]);
    while let Some((dir, level)) = stack.pop().filter(|_| out.len() < 256) {
        let read = (level < depth).then(|| std::fs::read_dir(&dir).into_iter().flatten().flatten());
        let below = read.into_iter().flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.'));
        stack
            .extend(below.filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| (e.path(), level + 1)));
        out.push(dir);
    }
    out
}

fn remote(dir: &Path) -> bool {
    rustix::fs::statfs(dir).is_ok_and(|s| REMOTE.contains(&((s.f_type as u64 & 0xffff_ffff) as u32)))
}

impl Watch {
    pub(super) fn new(dirs: Vec<PathBuf>, wake: Wake) -> Option<Watch> {
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).ok()?;
        let marked = dirs
            .iter()
            .filter(|d| inotify::add_watch(&fd, d.as_path(), crate::folders::flags()).is_ok())
            .count();
        if marked == 0 {
            return None;
        }
        let (missing, remote) = (marked < dirs.len(), dirs.iter().any(|d| remote(d)));
        let stop = Arc::new(eventfd(0, EventfdFlags::CLOEXEC).ok()?);
        let (changed, dead) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let (seen, gone, ended) = (Arc::clone(&changed), Arc::clone(&dead), Arc::clone(&stop));
        let reader = std::thread::Builder::new().name("wprs-listing".into());
        reader.spawn(move || read(&fd, &ended, &seen, &gone, &wake)).ok()?;
        Some(Watch { dirs, changed, dead, missing, remote, stop })
    }

    pub(super) fn whole(&self) -> bool {
        !self.missing && !self.dead.load(Ordering::Acquire)
    }

    pub(super) fn trusted(&self) -> bool {
        self.whole() && !self.remote
    }
}

fn read(fd: &OwnedFd, stop: &OwnedFd, changed: &AtomicBool, dead: &AtomicBool, wake: &Wake) {
    let lost = ReadFlags::IGNORED | ReadFlags::UNMOUNT | ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF;
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
        while let Ok(event) = reader.next() {
            any = true;
            if event.events().intersects(lost) {
                dead.store(true, Ordering::Release);
            }
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
