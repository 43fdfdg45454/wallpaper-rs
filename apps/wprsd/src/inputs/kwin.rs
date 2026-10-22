use std::path::PathBuf;
use tokio::sync::watch;

const KWIN: &str = "org.kde.KWin";
const SCRIPT: &str = "io.github.wallpaper_rs.pointer";
const SCRIPT_FILE: &str = "wallpaper-rs/kwin-pointer/contents/code/main.js";

fn script() -> Option<PathBuf> {
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let dirs = dirs::data_dir().into_iter().chain(std::env::split_paths(&system));
    dirs.map(|dir| dir.join(SCRIPT_FILE)).find(|path| path.is_file())
}

pub(crate) async fn kwin(connection: &zbus::Connection, on: bool) -> anyhow::Result<()> {
    let (path, interface) = ("/Scripting", Some("org.kde.kwin.Scripting"));
    connection.call_method(Some(KWIN), path, interface, "unloadScript", &(SCRIPT,)).await?;
    if on {
        let file = script().ok_or_else(|| anyhow::anyhow!("the KWin pointer script is not installed"))?;
        let load = (file.to_string_lossy().into_owned(), SCRIPT);
        connection.call_method(Some(KWIN), path, interface, "loadScript", &load).await?;
        connection.call_method(Some(KWIN), path, interface, "start", &()).await?;
    }
    Ok(())
}

pub(crate) async fn follow(connection: zbus::Connection, mut tracking: watch::Receiver<bool>) {
    loop {
        let on = *tracking.borrow_and_update();
        match kwin(&connection, on).await {
            Ok(()) => tracing::debug!(on, "KWin pointer script"),
            Err(e) if on => tracing::info!("no pointer position from KWin: {e:#}"),
            Err(e) => tracing::debug!("cannot unload the KWin pointer script: {e:#}"),
        }
        if tracking.changed().await.is_err() {
            return;
        }
    }
}
