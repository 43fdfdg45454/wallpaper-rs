use crate::daemon::{Daemon, State, output_name};
use crate::error::{invalid, not_found};
use crate::store::private_dir;
use crate::streams::OutputInfo;
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use std::collections::{HashMap, HashSet};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard, Weak};
use std::time::{Duration, Instant};
use wprs_core::{Edit, Fit, Place, Screen};
use wprs_wayland::Window;

const PREVIEW: Duration = Duration::from_secs(60);
const MAX_DIRS: usize = 256;

#[derive(Default)]
pub(crate) struct Sidecars {
    fd: Option<OwnedFd>,
    dirs: HashSet<PathBuf>,
    previews: HashMap<PathBuf, (Arc<Edit>, Instant)>,
}

fn flags() -> WatchFlags {
    WatchFlags::CLOSE_WRITE
        | WatchFlags::MOVED_TO
        | WatchFlags::MOVED_FROM
        | WatchFlags::DELETE
        | WatchFlags::ONLYDIR
}

pub(crate) fn spans(st: &State) -> bool {
    st.outputs.keys().any(|o| st.config.resolve(o).fit == Fit::Span)
}

pub(crate) fn screen_of(window: &Window) -> Screen {
    let (((x, y), (mm, _), identity), ((width, height), scale)) = (window.placement(), window.geometry());
    let px_per_mm = if mm > 0 { f64::from(window.size().0) / f64::from(mm) } else { 0.0 };
    Screen { x, y, width, height, scale: f64::from(scale) / 120.0, px_per_mm, identity: identity.into() }
}

impl OutputInfo {
    pub(crate) fn edited(&self) -> bool {
        self.uploaded.as_ref().is_some_and(|(.., edit, _)| edit.is_some())
    }
}

impl Daemon {
    pub(crate) fn edit_for(&self, file: &Path) -> Option<Arc<Edit>> {
        let mut st = self.state();
        let sidecars = &mut st.sidecars;
        if let (Some(fd), Some(dir)) = (&sidecars.fd, file.parent())
            && sidecars.dirs.len() < MAX_DIRS
            && sidecars.dirs.insert(dir.into())
        {
            inotify::add_watch(fd, dir, flags())
                .map(drop)
                .unwrap_or_else(|e| tracing::debug!("cannot watch: {e}"));
        }
        if let Some((edit, _)) = sidecars.previews.get(file) {
            return Some(edit.clone());
        }
        drop(st);
        match wprs_core::edit::load(file, &self.store.data.join("edits")) {
            Ok(found) => found.map(|(_, edit)| Arc::new(edit)),
            Err(e) => {
                tracing::warn!("ignoring an edit file that cannot be used");
                tracing::debug!("{e}");
                None
            }
        }
    }

    pub(crate) fn place(&self, output: &str, fit: Fit, edit: Option<&Edit>) -> Place {
        let st = self.state();
        let screen = st.outputs.get(output).and_then(|info| info.screen.as_ref());
        if let Some(crop) = edit.and_then(|e| e.crop_for(output, screen.map(|s| s.identity.as_str()))) {
            return Place::Crop(crop);
        }
        if fit != Fit::Span || screen.is_none() {
            return Place::Fit;
        }
        let spanned = st.outputs.iter().filter(|(name, _)| st.config.resolve(name).fit == Fit::Span);
        let screens =
            spanned.filter_map(|(name, info)| Some((name.as_str(), info.size, info.screen.as_ref()?)));
        let span = edit.and_then(|e| e.span.clone()).unwrap_or_default();
        let monitors = st.config.layout.arrange(screens);
        Place::Span { monitors, output: output.into(), center: span.center, zoom: span.zoom }
    }

    pub(crate) fn respan(&self, st: MutexGuard<'_, State>) {
        let spans = spans(&st);
        drop(st);
        if spans {
            self.configure_all();
        }
    }

    pub fn set_screen(&self, output: &str, screen: Screen) -> anyhow::Result<()> {
        let output = output_name(output)?;
        let fine = (0.1..=16.0).contains(&screen.scale) && (0.0..=1000.0).contains(&screen.px_per_mm);
        if !fine || screen.identity.len() > 256 || screen.identity.chars().any(char::is_control) {
            return Err(invalid("invalid scale, density or identity").into());
        }
        let changed = {
            let mut st = self.state();
            let info =
                st.outputs.get_mut(&output).ok_or_else(|| not_found(format!("{output} is not shown")))?;
            info.screen.replace(screen.clone()) != Some(screen)
        };
        if changed {
            self.configure_all();
            self.changes.changed("state", vec![output]);
        }
        Ok(())
    }

    pub fn preview_edit(self: &Arc<Self>, path: &Path, json: &str) -> anyhow::Result<()> {
        let path =
            std::fs::canonicalize(path).map_err(|_| not_found(format!("cannot find {}", path.display())))?;
        if json.len() > wprs_core::edit::MAX_BYTES {
            return Err(invalid("the edit is too large").into());
        }
        let edit =
            Some(json.trim()).filter(|j| !j.is_empty()).map(Edit::parse).transpose().map_err(invalid)?;
        let started = {
            let mut st = self.state();
            let previews = &mut st.sidecars.previews;
            match edit {
                Some(edit) => previews.insert(path.clone(), (Arc::new(edit), Instant::now())).is_none(),
                None => {
                    previews.remove(&path);
                    false
                }
            }
        };
        if started {
            tokio::spawn(expire(Arc::downgrade(self), path));
        }
        self.configure_all();
        Ok(())
    }
}

async fn expire(daemon: Weak<Daemon>, path: PathBuf) {
    while let Some(d) = daemon.upgrade() {
        let now = Instant::now();
        let due = {
            let mut st = d.state();
            let due = st.sidecars.previews.get(&path).map(|(_, at)| *at + PREVIEW);
            due.filter(|due| *due > now).or_else(|| st.sidecars.previews.remove(&path).map(|_| now))
        };
        match due {
            None => return,
            Some(due) if due <= now => {
                tracing::debug!("an edit preview expired");
                return drop(tokio::task::spawn_blocking(move || d.configure_all()));
            }
            Some(due) => {
                drop(d);
                tokio::time::sleep_until(due.into()).await
            }
        }
    }
}

pub fn start(daemon: &Arc<Daemon>) {
    let Ok(fd) = inotify::init(CreateFlags::CLOEXEC) else { return };
    let (data, cache) = (&daemon.store.data, &daemon.store.cache);
    for dir in [data.join("edits"), data.join("depth"), cache.join("depth")] {
        if private_dir(&dir).is_ok() {
            inotify::add_watch(&fd, &dir, flags())
                .map(drop)
                .unwrap_or_else(|e| tracing::debug!("cannot watch: {e}"));
        }
    }
    let Ok(reader) = fd.try_clone() else { return };
    daemon.state().sidecars.fd = Some(fd);
    let weak = Arc::downgrade(daemon);
    let thread = std::thread::Builder::new().name("wprs-edits".into()).spawn(move || read(&reader, &weak));
    thread.map(drop).unwrap_or_else(|e| tracing::warn!("cannot follow edit files: {e}"));
}

fn read(fd: &OwnedFd, daemon: &Weak<Daemon>) {
    let mut buffer = [MaybeUninit::uninit(); 4096];
    let mut reader = inotify::Reader::new(fd, &mut buffer);
    loop {
        let event = match reader.next() {
            Ok(event) => event,
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
        };
        let name = event.file_name().map(|n| n.to_bytes()).unwrap_or_default();
        let hex = name.len() >= 68 && name[..64].iter().all(u8::is_ascii_hexdigit);
        let edit = name.ends_with(b".wprs.json") || hex && name.len() == 69 && name.ends_with(b".json");
        if !edit && !name.ends_with(b".depth.png") && !(hex && name.ends_with(b".png")) {
            continue;
        }
        let Some(daemon) = daemon.upgrade() else { return };
        tracing::debug!("an edit file changed");
        daemon.configure_all();
    }
}
