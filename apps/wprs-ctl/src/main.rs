mod cli;

use anyhow::Context;
use clap::Parser;
use cli::{Cli, Command};
use wprs_core::dbus::BUS_NAME;
use zbus::fdo;

#[zbus::proxy(
    interface = "io.github.wallpaper_rs.Daemon1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
trait Daemon {
    fn request_stream(&self, output: &str, width: u32, height: u32) -> zbus::Result<(u32, u64)>;
    fn release_stream(&self, output: &str) -> zbus::Result<()>;
    fn set_wallpaper(&self, path: &str, output: &str, fit: &str) -> zbus::Result<()>;
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
    run(command, &daemon).await.map_err(|e| e.downcast::<zbus::Error>().map_or_else(|e| e, explain))
}

fn explain(error: zbus::Error) -> anyhow::Error {
    let not_running = || anyhow::anyhow!("wprsd is not running ({} is not on the session bus)", BUS_NAME);
    match error {
        zbus::Error::FDO(e) => match *e {
            fdo::Error::ServiceUnknown(_) | fdo::Error::NameHasNoOwner(_) => not_running(),
            fdo::Error::Failed(message) | fdo::Error::InvalidArgs(message) => anyhow::anyhow!(message),
            other => other.into(),
        },
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("ServiceUnknown") => not_running(),
        zbus::Error::MethodError(_, Some(message), _) => anyhow::anyhow!(message),
        other => other.into(),
    }
}

async fn run(command: Command, daemon: &DaemonProxy<'_>) -> anyhow::Result<()> {
    match command {
        Command::Set { path, output, fit } => {
            let path = std::path::absolute(&path)?;
            let path = path.to_str().context("the path is not valid UTF-8")?;
            daemon.set_wallpaper(path, &output, fit.map_or("", |f| f.as_str())).await?
        }
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
        Command::Stream { output, width, height } => {
            let (node, serial) = daemon.request_stream(&output, width, height).await?;
            println!("node {node} serial {serial}")
        }
        Command::Release { output } => daemon.release_stream(&output).await?,
        Command::Covered { output, covered } => daemon.set_output_covered(&output, covered).await?,
    }
    Ok(())
}

fn print_options(state: &str, filter: Option<&str>) -> anyhow::Result<()> {
    let state: serde_json::Value = serde_json::from_str(state)?;
    let options = state["options"].as_object().context("the daemon did not report its settings")?;
    options
        .iter()
        .filter(|(key, _)| filter.is_none_or(|f| key.as_str() == f))
        .for_each(|(key, value)| println!("{key} = {value}"));
    Ok(())
}
