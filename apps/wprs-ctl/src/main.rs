mod cli;

use anyhow::Context;
use clap::Parser;
use cli::{Cli, Command};
use std::path::Path;
use tokio::signal::unix::{SignalKind, signal};
use wprs_core::dbus::uri_scheme;

#[zbus::proxy(
    interface = "io.github.wallpaper_rs.Daemon1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
trait Daemon {
    fn request_stream(&self, output: &str, width: u32, height: u32) -> zbus::Result<(u32, u64)>;
    fn hold_stream(&self, output: &str, width: u32, height: u32) -> zbus::Result<(u32, u64)>;
    fn release_stream(&self, output: &str) -> zbus::Result<()>;
    fn set_wallpaper(&self, path: &str, output: &str, fit: &str) -> zbus::Result<()>;
    fn set_background(&self, color: &str, output: &str) -> zbus::Result<()>;
    fn set_option(&self, key: &str, value: &str) -> zbus::Result<()>;
    fn reload(&self) -> zbus::Result<()>;
    fn get_state(&self) -> zbus::Result<String>;
    fn set_paused(&self, paused: bool) -> zbus::Result<()>;
    fn set_output_covered(&self, output: &str, covered: bool) -> zbus::Result<()>;
    fn set_volume(&self, volume: f64) -> zbus::Result<()>;
    fn set_muted(&self, muted: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn paused(&self) -> zbus::Result<bool>;
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let command = Cli::parse().command;
    let connection = zbus::Connection::session().await.context("cannot connect to the session bus")?;
    let daemon = DaemonProxy::new(&connection).await?;
    run(command, &daemon).await.map_err(|e| e.downcast::<zbus::Error>().map_or_else(|e| e, cli::explain))
}

async fn run(command: Command, daemon: &DaemonProxy<'_>) -> anyhow::Result<()> {
    match command {
        Command::Set { path, output, fit } => {
            daemon.set_wallpaper(&wallpaper(&path)?, &output, fit.map_or("", |f| f.as_str())).await?
        }
        Command::Fit { fit, output } => daemon.set_wallpaper("", &output, fit.as_str()).await?,
        Command::Background { color, output } => daemon.set_background(&color, &output).await?,
        Command::Config { key: Some(key), value: Some(value) } => daemon.set_option(&key, &value).await?,
        Command::Config { key, .. } => print_options(&daemon.get_state().await?, key.as_deref())?,
        Command::Reload => daemon.reload().await?,
        Command::State => println!("{}", daemon.get_state().await?),
        Command::Pause => daemon.set_paused(true).await?,
        Command::Resume => daemon.set_paused(false).await?,
        Command::Toggle => daemon.set_paused(!daemon.paused().await?).await?,
        Command::Volume { percent } => {
            daemon.set_volume(f64::from(percent) / 100.0).await?;
            daemon.set_muted(false).await?
        }
        Command::Mute => daemon.set_muted(true).await?,
        Command::Unmute => daemon.set_muted(false).await?,
        Command::Stream { output, width, height, wait } => {
            let interrupted = wait.then(interrupted).transpose()?;
            let (node, serial) = match wait {
                true => daemon.request_stream(&output, width, height).await?,
                false => daemon.hold_stream(&output, width, height).await?,
            };
            println!("node {node} serial {serial}");
            if let Some(interrupted) = interrupted {
                interrupted.await;
                daemon.release_stream(&output).await?
            }
        }
        Command::Release { output } => daemon.release_stream(&output).await?,
        Command::Covered { output, covered } => daemon.set_output_covered(&output, covered).await?,
    }
    Ok(())
}

fn wallpaper(path: &Path) -> anyhow::Result<String> {
    let text = path.to_str().context("the path is not valid UTF-8")?;
    match uri_scheme(text).filter(|_| !path.exists()) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => Ok(text.into()),
        Some(_) => anyhow::bail!("{text} is not a local file"),
        None => Ok(std::path::absolute(path)?.to_str().context("the path is not valid UTF-8")?.into()),
    }
}

fn interrupted() -> std::io::Result<impl Future<Output = ()>> {
    let (mut interrupt, mut terminate) = (signal(SignalKind::interrupt())?, signal(SignalKind::terminate())?);
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    })
}

fn print_options(state: &str, filter: Option<&str>) -> anyhow::Result<()> {
    let state: serde_json::Value = serde_json::from_str(state)?;
    let options = state["options"].as_object().context("the daemon did not report its settings")?;
    if let Some(key) = filter.filter(|key| !options.contains_key(*key)) {
        let known: Vec<&str> = options.keys().map(String::as_str).collect();
        anyhow::bail!("unknown setting '{key}'; the settings are {}", known.join(", "));
    }
    (options.iter())
        .filter(|(key, _)| filter.is_none_or(|f| key.as_str() == f))
        .for_each(|(key, value)| println!("{key} = {value}"));
    Ok(())
}
