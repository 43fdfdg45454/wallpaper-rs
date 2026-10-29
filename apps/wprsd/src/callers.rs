use crate::compositor::lock;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use zbus::fdo::DBusProxy;
use zbus::message::Header;

const KWIN: &str = "org.kde.KWin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caller {
    Ctl,
    Ours,
    Compositor,
    Other,
}

static KNOWN: LazyLock<Mutex<HashMap<String, Caller>>> = LazyLock::new(Default::default);

pub(crate) async fn caller(bus: &zbus::Connection, header: &Header<'_>) -> Caller {
    let Some(sender) = header.sender().map(|s| s.to_string()) else { return Caller::Other };
    if let Some(known) = lock(&KNOWN).get(&sender) {
        return *known;
    }
    let found = identify(bus, &sender).await.unwrap_or(Caller::Other);
    tracing::debug!(?found, "a caller of a protected method");
    lock(&KNOWN).insert(sender, found);
    found
}

pub(crate) fn forget(name: &str) {
    lock(&KNOWN).remove(name);
}

pub(crate) fn reports_pointer(caller: Caller) -> bool {
    caller == Caller::Ctl || caller == Caller::Compositor && crate::inputs::kwin_script_loaded()
}

async fn identify(bus: &zbus::Connection, sender: &str) -> zbus::fdo::Result<Caller> {
    let dbus = DBusProxy::new(bus).await?;
    let kwin = dbus.get_name_owner(KWIN.try_into()?).await.is_ok_and(|owner| owner.as_str() == sender);
    if crate::flatpak::app_id().is_some() {
        return Ok(if kwin { Caller::Compositor } else { Caller::Other });
    }
    let pid = dbus.get_connection_unix_process_id(sender.try_into()?).await?;
    let found = move || if kwin { compositor(pid) } else { of(pid, &ours()) };
    Ok(tokio::task::spawn_blocking(found).await.unwrap_or(Caller::Other))
}

fn system(path: impl AsRef<Path>) -> bool {
    path.as_ref().metadata().is_ok_and(|m| m.uid() == 0 && m.mode() & 0o022 == 0)
}

fn compositor(pid: u32) -> Caller {
    let exe = PathBuf::from(format!("/proc/{pid}/exe"));
    let name =
        std::fs::read_link(&exe).ok().and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned()));
    let kwin = name.is_some_and(|n| ["kwin_wayland", "kwin_x11"].contains(&n.trim_end_matches(" (deleted)")));
    if kwin && system(&exe) { Caller::Compositor } else { Caller::Other }
}

fn ours() -> Vec<(PathBuf, Caller)> {
    let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let tools = [("wprs-ctl", Caller::Ctl), ("wprs-app", Caller::Ours)];
    let tools = dir.into_iter().flat_map(|dir| tools.map(|(name, caller)| (dir.join(name), caller)));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let shell = std::env::split_paths(&path).map(|dir| dir.join("plasmashell")).find(|p| system(p));
    tools.chain(shell.map(|shell| (shell, Caller::Ours))).collect()
}

fn of(pid: u32, trusted: &[(PathBuf, Caller)]) -> Caller {
    let id = |m: std::fs::Metadata| (m.dev(), m.ino());
    let Ok(exe) = std::fs::metadata(format!("/proc/{pid}/exe")).map(id) else { return Caller::Other };
    let found = trusted.iter().find(|(p, _)| p.metadata().is_ok_and(|m| id(m) == exe));
    found.map_or(Caller::Other, |(_, caller)| *caller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confirm::tests::{connect, private_bus};

    #[test]
    fn callers_are_ours_only_when_they_run_one_of_the_trusted_files() {
        let (me, exe) = (std::process::id(), std::env::current_exe().unwrap());
        assert_eq!(of(me, &[(exe.clone(), Caller::Ctl)]), Caller::Ctl);
        assert_eq!(of(me, &[(exe.clone(), Caller::Ours)]), Caller::Ours);
        assert_eq!(of(me, &[(PathBuf::from("/nonexistent/wprs-ctl"), Caller::Ctl)]), Caller::Other);
        assert_eq!(of(u32::MAX, &[(exe, Caller::Ctl)]), Caller::Other);
        let tools = ours();
        assert!(tools.iter().take(2).all(|t| t.0.parent() == std::env::current_exe().unwrap().parent()));
        assert_eq!(tools.iter().take(2).map(|t| t.1).collect::<Vec<_>>(), [Caller::Ctl, Caller::Ours]);
        assert!(
            !reports_pointer(Caller::Ours) && !reports_pointer(Caller::Other) && reports_pointer(Caller::Ctl)
        );
        assert!(
            !reports_pointer(Caller::Compositor),
            "a compositor was trusted before its script was loaded"
        );
    }

    #[tokio::test]
    async fn a_program_that_takes_the_name_of_kwin_is_not_the_compositor() {
        let Some(bus) = private_bus() else { return };
        let (daemon, fake) = (connect(&bus.1).await, connect(&bus.1).await);
        fake.request_name(KWIN).await.unwrap();
        let sender = fake.unique_name().unwrap().to_string();
        assert_eq!(identify(&daemon, &sender).await.unwrap(), Caller::Other);
        assert_eq!(compositor(std::process::id()), Caller::Other);
    }
}
