use crate::daemon::Daemon;
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::io::Errno;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use zbus::{OwnedObjectPath, Variant};

const MONITOR: &str = "/run/host/monitor";

#[zbus::proxy(
    interface = "org.freedesktop.portal.Background",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Background {
    fn request_background(
        &self,
        parent_window: &str,
        options: HashMap<&str, Variant<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

pub fn app_id() -> Option<String> {
    std::env::var("FLATPAK_ID").ok().filter(|id| !id.is_empty())
}

pub fn host_path(path: &Path) -> PathBuf {
    let info = app_id().and_then(|_| std::fs::read_to_string("/.flatpak-info").ok()).unwrap_or_default();
    let app = info.lines().find_map(|line| line.strip_prefix("app-path="));
    match (app, path.strip_prefix("/app")) {
        (Some(app), Ok(rest)) => Path::new(app).join(rest),
        _ => path.to_path_buf(),
    }
}

pub async fn request_background(connection: zbus::Connection) {
    if app_id().is_none() {
        return;
    }
    let options = HashMap::from([("reason", Variant::from("Draws the desktop wallpaper"))]);
    let request = async { BackgroundProxy::new(&connection).await?.request_background("", options).await };
    if let Err(e) = request.await {
        tracing::warn!("cannot ask the background portal to keep wprsd running: {e}");
    }
}

pub fn follow_zone(daemon: Weak<Daemon>) -> std::io::Result<()> {
    watch(Path::new(MONITOR), "timezone", move |zone| {
        crate::zone::follow(zone.trim());
        daemon.upgrade().map(|daemon| daemon.rotations.retime()).is_some()
    })
}

fn watch(
    dir: &Path,
    name: &'static str,
    changed: impl Fn(&str) -> bool + Send + 'static,
) -> std::io::Result<()> {
    let fd = inotify::init(CreateFlags::CLOEXEC)?;
    inotify::add_watch(&fd, dir, WatchFlags::CLOSE_WRITE | WatchFlags::MOVED_TO)?;
    let path = dir.join(name);
    let watch = move || {
        let mut buffer = [MaybeUninit::uninit(); 1024];
        let mut reader = inotify::Reader::new(&fd, &mut buffer);
        loop {
            match reader.next() {
                Ok(event) if event.file_name().is_some_and(|n| n.to_bytes() == name.as_bytes()) => {
                    if std::fs::read_to_string(&path).is_ok_and(|text| !changed(&text)) {
                        return;
                    }
                }
                Ok(_) | Err(Errno::INTR) => {}
                Err(_) => return,
            }
        }
    };
    std::thread::Builder::new().name("wprs-zone".into()).spawn(watch).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_zone_file_is_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        watch(dir.path(), "timezone", move |zone| tx.send(zone.to_owned()).is_ok()).unwrap();
        std::fs::write(dir.path().join("other"), "Etc/UTC\n").unwrap();
        std::fs::write(dir.path().join("timezone.new"), "Etc/GMT-2\n").unwrap();
        std::fs::rename(dir.path().join("timezone.new"), dir.path().join("timezone")).unwrap();
        let read = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(read, "Etc/GMT-2\n");
    }
}
