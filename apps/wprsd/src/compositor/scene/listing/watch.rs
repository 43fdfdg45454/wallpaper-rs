use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub(in crate::compositor) type Wake = Arc<dyn Fn() + Send + Sync>;

pub(super) const MOST: usize = 8192;
const SETTLE: Duration = Duration::from_millis(500);
const LONGEST: Duration = Duration::from_secs(5);

pub(in crate::compositor) struct Watch {
    pub(super) dirs: Vec<PathBuf>,
    pub(super) changed: Arc<AtomicBool>,
    heard: Arc<[AtomicU64; 2]>,
    born: Instant,
    dead: Arc<AtomicBool>,
    missing: bool,
    waiting: bool,
    remote: bool,
    stop: Arc<OwnedFd>,
}

pub(super) fn dirs(top: &Path, depth: u8) -> (Vec<PathBuf>, bool) {
    let (mut out, mut stack) = (Vec::new(), vec![(top.to_owned(), 0u8)]);
    while let Some((dir, level)) = stack.pop() {
        if out.len() == MOST {
            return (out, true);
        }
        let read = (level < depth).then(|| std::fs::read_dir(&dir).into_iter().flatten().flatten());
        let below = read.into_iter().flatten().filter(|e| !e.file_name().as_bytes().starts_with(b"."));
        stack
            .extend(below.filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| (e.path(), level + 1)));
        out.push(dir);
    }
    (out, false)
}

struct Marks {
    full: HashSet<i32>,
    awaited: HashMap<i32, Vec<OsString>>,
}

fn ancestor(fd: &OwnedFd, dir: &Path) -> Option<(i32, OsString)> {
    let flags = WatchFlags::CREATE | WatchFlags::MOVED_TO | WatchFlags::ONLYDIR | WatchFlags::MASK_ADD;
    let mut below = dir;
    for up in dir.ancestors().skip(1) {
        if let Ok(wd) = inotify::add_watch(fd, up, flags) {
            return Some((wd, below.file_name()?.to_owned()));
        }
        below = up;
    }
    None
}

impl Watch {
    pub(super) fn new((dirs, cut): (Vec<PathBuf>, bool), wake: Wake) -> Option<Watch> {
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).ok()?;
        let mut marks = Marks { full: HashSet::new(), awaited: HashMap::new() };
        let (mut missing, mut waiting) = (cut, false);
        for d in &dirs {
            match inotify::add_watch(&fd, d.as_path(), crate::folders::flags()) {
                Ok(wd) => {
                    marks.full.insert(wd);
                }
                Err(Errno::ACCESS | Errno::PERM) => {}
                Err(Errno::NOENT | Errno::NOTDIR) => match ancestor(&fd, d) {
                    Some((wd, name)) => {
                        marks.awaited.entry(wd).or_default().push(name);
                        waiting = true;
                    }
                    None => missing = true,
                },
                Err(_) => missing = true,
            }
        }
        if marks.full.is_empty() && marks.awaited.is_empty() {
            return None;
        }
        let remote = dirs.iter().any(|d| super::mounts::remote(d));
        let stop = Arc::new(eventfd(0, EventfdFlags::CLOEXEC).ok()?);
        let (changed, dead) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let (heard, born) = (Arc::new([AtomicU64::new(0), AtomicU64::new(0)]), Instant::now());
        let shared = (Arc::clone(&changed), Arc::clone(&dead), Arc::clone(&heard));
        let ended = Arc::clone(&stop);
        let reader = std::thread::Builder::new().name("wprs-listing".into());
        reader.spawn(move || read(&fd, &ended, &marks, (&shared, born), &wake)).ok()?;
        Some(Watch { dirs, changed, heard, born, dead, missing, waiting, remote, stop })
    }

    #[cfg(test)]
    pub(super) fn awaiting(&self) -> bool {
        self.waiting && !self.dead.load(Ordering::Acquire)
    }

    pub(super) fn whole(&self) -> bool {
        !self.waiting && !self.dead.load(Ordering::Acquire)
    }

    pub(super) fn trusted(&self) -> bool {
        !self.missing && !self.remote
    }

    pub(super) fn pending(&self, now: Instant) -> Option<f64> {
        if !self.changed.load(Ordering::Acquire) {
            return None;
        }
        let at = |i: usize| self.born + Duration::from_millis(self.heard[i].load(Ordering::Acquire));
        let due = (at(0) + SETTLE).min(at(1) + LONGEST);
        Some(due.saturating_duration_since(now).as_secs_f64())
    }

    pub(super) fn take(&self, now: Instant) -> bool {
        self.pending(now) == Some(0.0) && self.changed.swap(false, Ordering::AcqRel)
    }
}

type Shared = (Arc<AtomicBool>, Arc<AtomicBool>, Arc<[AtomicU64; 2]>);

fn read(
    fd: &OwnedFd,
    stop: &OwnedFd,
    marks: &Marks,
    ((changed, dead, heard), born): (&Shared, Instant),
    wake: &Wake,
) {
    let lost = ReadFlags::IGNORED | ReadFlags::UNMOUNT | ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF;
    let mut buffer = [MaybeUninit::uninit(); 4096];
    loop {
        let mut fds = [PollFd::new(fd, PollFlags::IN), PollFd::new(stop, PollFlags::IN)];
        match poll(&mut fds, None) {
            Err(Errno::INTR) => continue,
            Err(_) => return,
            Ok(_) if !fds[1].revents().is_empty() => return,
            Ok(_) => {}
        }
        let mut reader = inotify::Reader::new(fd, &mut buffer);
        let mut any = false;
        while let Ok(event) = reader.next() {
            let name = event.file_name().map(|n| n.to_bytes());
            let counts = match marks.awaited.get(&event.wd()) {
                _ if event.events().intersects(lost) => {
                    dead.store(true, Ordering::Release);
                    true
                }
                Some(names) if !marks.full.contains(&event.wd()) => {
                    name.is_some_and(|n| names.iter().any(|w| w.as_bytes() == n))
                }
                _ => name.is_none_or(|n| !n.starts_with(b".")),
            };
            any |= counts;
        }
        if !any {
            continue;
        }
        let now = born.elapsed().as_millis() as u64;
        if !changed.load(Ordering::Acquire) {
            heard[1].store(now, Ordering::Release);
        }
        heard[0].store(now, Ordering::Release);
        if !changed.swap(true, Ordering::AcqRel) {
            wake();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = rustix::io::write(&*self.stop, &1u64.to_ne_bytes());
    }
}
