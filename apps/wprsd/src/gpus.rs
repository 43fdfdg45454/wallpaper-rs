use crate::compositor::lock;
use crate::daemon::Daemon;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::io::Errno;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use wprs_core::GpuChoice;
use wprs_core::model::GpuAdapter;
use wprs_render::{Chosen, Fallback, Renderer};

const NODES: &str = "/dev/dri";
const SETTLE: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct Gpus {
    adapters: Mutex<Vec<GpuAdapter>>,
    pub(crate) warning: Option<String>,
    started: GpuChoice,
    compositor: Option<String>,
    pub(crate) renderer: Option<&'static Renderer>,
}

impl Gpus {
    pub(crate) fn start(chosen: Chosen, choice: &GpuChoice, compositor: Option<String>) -> (Gpus, Renderer) {
        let warning = chosen.fallback.map(|why| match why {
            Fallback::Missing => crate::words::gpu_missing(choice.as_str()),
            Fallback::Unusable => crate::words::gpu_unusable(choice.as_str()),
        });
        let adapters = Mutex::new(chosen.adapters);
        let gpus = Gpus { adapters, warning, started: choice.clone(), compositor, renderer: None };
        (gpus, chosen.renderer)
    }

    pub(crate) fn adapters(&self) -> Vec<GpuAdapter> {
        let mut found = lock(&self.adapters).clone();
        let bus = self.compositor.as_deref();
        found.iter_mut().for_each(|a| a.compositor = bus.is_some_and(|b| a.bus == b));
        found
    }

    pub(crate) fn note(&self) -> Option<String> {
        let bus = self.compositor.as_deref()?;
        let copies = !lock(&self.adapters).iter().any(|a| a.used && a.bus == bus);
        copies.then(crate::words::gpu_copies)
    }

    pub(crate) fn restart(&self, configured: &GpuChoice) -> bool {
        !configured.same(&self.started)
    }

    fn refresh(&self) -> bool {
        let Some(renderer) = self.renderer else { return false };
        let now = renderer.plugged();
        std::mem::replace(&mut *lock(&self.adapters), now.clone()) != now
    }
}

pub(crate) fn watch(daemon: &Arc<Daemon>) {
    let Ok(fd) = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK) else { return };
    if inotify::add_watch(&fd, NODES, WatchFlags::CREATE | WatchFlags::DELETE).is_err() {
        return;
    }
    let weak = Arc::downgrade(daemon);
    let thread = std::thread::Builder::new().name("wprs-gpus".into()).spawn(move || follow(&fd, &weak));
    thread.map(drop).unwrap_or_else(|e| tracing::warn!("cannot follow graphics cards: {e}"));
}

pub(crate) fn card(name: &[u8]) -> bool {
    let digits = |rest: &[u8]| !rest.is_empty() && rest.iter().all(u8::is_ascii_digit);
    name.strip_prefix(b"card").is_some_and(digits) || name.strip_prefix(b"renderD").is_some_and(digits)
}

fn drained(reader: &mut inotify::Reader<'_, &OwnedFd>) -> Option<bool> {
    let mut seen = false;
    loop {
        match reader.next() {
            Ok(event) => seen |= event.file_name().is_some_and(|n| card(n.to_bytes())),
            Err(Errno::INTR) => {}
            Err(Errno::AGAIN) => return Some(seen),
            Err(_) => return None,
        }
    }
}

fn follow(fd: &OwnedFd, daemon: &Weak<Daemon>) {
    let mut buffer = [MaybeUninit::uninit(); 1024];
    let mut reader = inotify::Reader::new(fd, &mut buffer);
    loop {
        match poll(&mut [PollFd::new(fd, PollFlags::IN)], None) {
            Ok(_) | Err(Errno::INTR) => {}
            Err(_) => return,
        }
        let Some(seen) = drained(&mut reader) else { return };
        if !seen {
            continue;
        }
        std::thread::sleep(SETTLE);
        if drained(&mut reader).is_none() {
            return;
        }
        let Some(daemon) = daemon.upgrade() else { return };
        if daemon.gpus.refresh() {
            tracing::info!("the graphics cards changed");
            daemon.changes.changed("state", Vec::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GpuAdapter, Gpus};
    use std::sync::Mutex;

    fn card(bus: &str, used: bool) -> GpuAdapter {
        let (name, kind, backend) = ("Example".to_owned(), "dedicated".to_owned(), "vulkan".to_owned());
        GpuAdapter { name, kind, backend, used, bus: bus.into(), compositor: false }
    }

    fn gpus(compositor: Option<&str>) -> Gpus {
        let adapters = Mutex::new(vec![card("0000:00:02.0", true), card("0000:03:00.0", false)]);
        Gpus { adapters, compositor: compositor.map(Into::into), ..Gpus::default() }
    }

    #[test]
    fn the_card_of_the_compositor_is_marked_and_a_note_tells_when_it_is_not_the_one_in_use() {
        let other = gpus(Some("0000:03:00.0"));
        assert_eq!(other.adapters().iter().map(|a| a.compositor).collect::<Vec<_>>(), [false, true]);
        assert!(other.note().is_some_and(|note| note.contains("copied")));
        let same = gpus(Some("0000:00:02.0"));
        assert_eq!(same.adapters().iter().map(|a| a.compositor).collect::<Vec<_>>(), [true, false]);
        assert_eq!(same.note(), None, "the card in use draws the desktop");
        let unknown = gpus(None);
        assert!(unknown.adapters().iter().all(|a| !a.compositor) && unknown.note().is_none());
    }

    #[test]
    fn only_card_and_render_nodes_count_as_graphics_cards() {
        let named = [&b"card0"[..], b"card12", b"renderD128", b"card", b"by-path", b"card0-DP-1", b"renderD"];
        assert_eq!(named.map(super::card), [true, true, true, false, false, false, false]);
    }

    #[test]
    fn a_card_and_its_render_node_appearing_together_count_once() {
        use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
        let dir = tempfile::tempdir().unwrap();
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).unwrap();
        inotify::add_watch(&fd, dir.path(), WatchFlags::CREATE | WatchFlags::DELETE).unwrap();
        let mut buffer = [std::mem::MaybeUninit::uninit(); 1024];
        let mut reader = inotify::Reader::new(&fd, &mut buffer);
        assert_eq!(super::drained(&mut reader), Some(false), "nothing happened yet");
        for name in ["card1", "renderD129", "by-path"] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        assert_eq!(super::drained(&mut reader), Some(true));
        assert_eq!(super::drained(&mut reader), Some(false), "one burst was counted twice");
        std::fs::write(dir.path().join("by-id"), b"").unwrap();
        assert_eq!(super::drained(&mut reader), Some(false), "a name that is no card counted");
    }
}
