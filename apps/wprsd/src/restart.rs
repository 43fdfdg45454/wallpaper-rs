use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use zbus::OwnedObjectPath;

static ASKED: AtomicBool = AtomicBool::new(false);
static WAKE: Notify = Notify::const_new();
const REPLY: Duration = Duration::from_millis(200);
const SYSTEMD: &str = "org.freedesktop.systemd1";

async fn systemd(bus: &zbus::Connection) -> zbus::Result<bool> {
    let manager =
        zbus::Proxy::new(bus, SYSTEMD, "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager")
            .await?;
    let unit: OwnedObjectPath = manager.call("GetUnitByPID", &(std::process::id(),)).await?;
    let service = zbus::Proxy::new(bus, SYSTEMD, unit.clone(), "org.freedesktop.systemd1.Service").await?;
    if service.get_property::<u32>("MainPID").await? != std::process::id() {
        return Ok(false);
    }
    let unit = zbus::Proxy::new(bus, SYSTEMD, unit, "org.freedesktop.systemd1.Unit").await?;
    let _job: OwnedObjectPath = unit.call("Restart", &("replace",)).await?;
    Ok(true)
}

pub(crate) async fn ask(bus: &zbus::Connection) {
    if std::env::var_os("INVOCATION_ID").is_some() {
        match systemd(bus).await {
            Ok(true) => return tracing::info!("systemd starts the daemon again"),
            Ok(false) => tracing::debug!("the daemon is not the main process of its unit"),
            Err(e) => tracing::debug!("systemd did not restart the daemon: {e}"),
        }
    }
    tokio::spawn(async {
        tokio::time::sleep(REPLY).await;
        ASKED.store(true, Ordering::SeqCst);
        WAKE.notify_one();
    });
}

pub(crate) async fn asked() {
    WAKE.notified().await
}

pub(crate) fn wanted() -> bool {
    ASKED.load(Ordering::SeqCst)
}

pub(crate) fn again() {
    let Ok(program) = std::env::current_exe() else {
        return tracing::error!("cannot find the daemon to start again");
    };
    let text = program.to_string_lossy();
    let program = text.strip_suffix(" (deleted)").map_or(program.clone(), std::path::PathBuf::from);
    let mut args = std::env::args_os();
    let mut command = std::process::Command::new(program);
    if let Some(first) = args.next() {
        command.arg0(first);
    }
    tracing::info!("starting the daemon again");
    let error = command.args(args).exec();
    tracing::error!("cannot start the daemon again: {error}");
}
