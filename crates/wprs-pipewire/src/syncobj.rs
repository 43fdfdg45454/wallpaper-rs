use crate::params::META_TIMELINE;
use pipewire::sys::pw_buffer;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

const fn readwrite(nr: u64, size: usize) -> u64 {
    (3 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | nr
}

const GET_CAP: u64 = readwrite(0x0c, size_of::<Cap>());
const CREATE: u64 = readwrite(0xbf, size_of::<Create>());
const DESTROY: u64 = readwrite(0xc0, size_of::<Create>());
const EXPORT: u64 = readwrite(0xc1, size_of::<Export>());
const QUERY: u64 = readwrite(0xcb, size_of::<Points>());
const SIGNAL: u64 = readwrite(0xcd, size_of::<Points>());
const CAP_TIMELINE: u64 = 0x14;
const RELEASE_WAIT: Duration = Duration::from_secs(1);
pub(crate) const UNSCHEDULED_RELEASE: u32 = 1;

#[repr(C)]
struct Cap(u64, u64);

#[repr(C)]
struct Create(u32, u32);

#[repr(C)]
struct Export(u32, u32, i32, u32);

#[repr(C)]
struct Points(u64, u64, u32, u32);

pub trait Timeline: Send + Sync {
    fn signal(&self, point: u64) -> io::Result<()>;
    fn reached(&self) -> io::Result<u64>;
}

#[repr(C)]
#[derive(Default)]
pub(crate) struct SyncMeta {
    pub(crate) flags: u32,
    padding: u32,
    pub(crate) acquire: u64,
    pub(crate) release: u64,
}

pub struct Fences {
    acquire: Box<dyn Timeline>,
    release: Box<dyn Timeline>,
    point: u64,
    waiting: Option<Instant>,
}

impl Fences {
    pub fn new(acquire: Box<dyn Timeline>, release: Box<dyn Timeline>) -> Fences {
        Fences { acquire, release, point: 0, waiting: None }
    }

    pub(crate) fn free(&self) -> bool {
        let released = || self.release.reached().is_ok_and(|reached| reached >= self.point);
        self.waiting.is_none_or(|since| since.elapsed() >= RELEASE_WAIT || released())
    }

    pub(crate) fn returned(&mut self, meta: Option<&SyncMeta>) {
        let scheduled = meta.is_some_and(|m| m.flags & UNSCHEDULED_RELEASE == 0);
        self.waiting = scheduled.then(Instant::now);
    }

    pub(crate) fn sent(&mut self, meta: Option<&mut SyncMeta>) {
        self.point += 1;
        if let Err(e) = self.acquire.signal(self.point) {
            tracing::warn!("cannot signal a finished stream frame: {e}");
        }
        if let Some(meta) = meta {
            (meta.flags, meta.acquire, meta.release) =
                (meta.flags | UNSCHEDULED_RELEASE, self.point, self.point);
        }
    }
}

pub(crate) unsafe fn sync_meta<'a>(buffer: *mut pw_buffer) -> Option<&'a mut SyncMeta> {
    let spa = unsafe { buffer.as_ref()?.buffer.as_ref()? };
    if spa.metas.is_null() {
        return None;
    }
    let metas = unsafe { std::slice::from_raw_parts(spa.metas, spa.n_metas as usize) };
    let found =
        metas.iter().find(|m| m.type_ == META_TIMELINE && m.size as usize >= size_of::<SyncMeta>())?;
    unsafe { found.data.cast::<SyncMeta>().as_mut() }
}

pub struct Drm(File);

pub struct Syncobj {
    drm: Arc<Drm>,
    handle: u32,
}

fn call<T>(file: &File, request: u64, arg: &mut T) -> io::Result<()> {
    match unsafe { libc::ioctl(file.as_raw_fd(), request as _, std::ptr::from_mut(arg)) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

impl Drm {
    pub fn open() -> Option<Arc<Drm>> {
        let node = |minor: u32| {
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_CLOEXEC)
                .open(format!("/dev/dri/renderD{minor}"))
        };
        let timelines = |file: &File| {
            let mut cap = Cap(CAP_TIMELINE, 0);
            call(file, GET_CAP, &mut cap).is_ok() && cap.1 == 1
        };
        (128..192).filter_map(|minor| node(minor).ok()).find(timelines).map(|file| Arc::new(Drm(file)))
    }
}

impl Syncobj {
    pub fn new(drm: &Arc<Drm>) -> io::Result<Syncobj> {
        let mut create = Create(0, 0);
        call(&drm.0, CREATE, &mut create)?;
        Ok(Syncobj { drm: drm.clone(), handle: create.0 })
    }

    pub fn export(&self) -> io::Result<OwnedFd> {
        let mut export = Export(self.handle, 0, -1, 0);
        call(&self.drm.0, EXPORT, &mut export)?;
        Ok(unsafe { OwnedFd::from_raw_fd(export.2) })
    }

    fn points(&self, request: u64, point: u64) -> io::Result<u64> {
        let (handle, mut point) = (u64::from(self.handle), point);
        let mut points =
            Points(std::ptr::from_ref(&handle) as u64, std::ptr::from_mut(&mut point) as u64, 1, 0);
        call(&self.drm.0, request, &mut points)?;
        Ok(point)
    }
}

impl Timeline for Syncobj {
    fn signal(&self, point: u64) -> io::Result<()> {
        self.points(SIGNAL, point).map(drop)
    }

    fn reached(&self) -> io::Result<u64> {
        self.points(QUERY, 0)
    }
}

impl Drop for Syncobj {
    fn drop(&mut self) {
        let _ = call(&self.drm.0, DESTROY, &mut Create(self.handle, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_match_the_kernel_interface() {
        assert_eq!(
            [GET_CAP, CREATE, DESTROY, EXPORT, QUERY, SIGNAL],
            [0xc010640c, 0xc00864bf, 0xc00864c0, 0xc01064c1, 0xc01864cb, 0xc01864cd]
        );
        let Some(drm) = Drm::open() else { return };
        let timeline = Syncobj::new(&drm).unwrap();
        assert!(timeline.export().is_ok());
        assert_eq!(timeline.reached().unwrap(), 0);
        timeline.signal(3).unwrap();
        assert_eq!(timeline.reached().unwrap(), 3);
    }
}
