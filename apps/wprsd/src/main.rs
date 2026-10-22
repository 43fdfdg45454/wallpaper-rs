mod bus;
mod compositor;
mod controls;
mod daemon;
mod flatpak;
mod service;
mod streams;
mod wallpaper;
mod watch;

use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use wprs_core::{Config, dbus};
use zbus::fdo::{DBusProxy, RequestNameFlags};

#[derive(Parser)]
#[command(version, about = "Renders wallpapers and serves them to the desktop over D-Bus and PipeWire")]
struct Args {
    #[arg(long, help = "Configuration file to use instead of the default one")]
    config: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    skip_vulkan_device_selection_layer();
    run(args)
}

fn skip_vulkan_device_selection_layer() {
    if std::env::var_os("NODEVICE_SELECT").is_none() {
        unsafe { std::env::set_var("NODEVICE_SELECT", "1") };
    }
}

#[tokio::main]
async fn run(args: Args) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let config_path = args.config.map_or_else(Config::default_path, Ok)?;
    let connection = zbus::Connection::session().await.context("cannot connect to the session bus")?;
    let bus = DBusProxy::new(&connection).await?;
    let taken = || anyhow::anyhow!("another wprsd already owns {} on the session bus", dbus::BUS_NAME);
    if bus.name_has_owner(dbus::BUS_NAME.try_into()?).await? {
        return Err(taken());
    }
    let daemon = Arc::new(tokio::task::spawn_blocking(move || daemon::Daemon::new(config_path)).await??);
    let vanished = bus.receive_name_owner_changed_with_args(&[(2, "")]).await?;
    tokio::spawn(watch::clients(vanished, daemon.clone()));
    connection.object_server().at(dbus::OBJECT_PATH, service::Service(daemon.clone())).await?;
    let flags = RequestNameFlags::DO_NOT_QUEUE;
    connection.request_name_with_flags(dbus::BUS_NAME, flags).await.map_err(|e| match e {
        zbus::Error::NameTaken => taken(),
        e => e.into(),
    })?;
    tracing::info!("serving {} on the session bus", dbus::BUS_NAME);
    tokio::spawn(flatpak::request_background(connection.clone()));
    tokio::spawn(watch::battery(daemon.clone()));
    tokio::spawn(watch::screen_lock(connection.clone(), daemon.clone()));
    tokio::spawn(watch::properties(connection.clone(), daemon.clone()));
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    tracing::info!("shutting down");
    Ok(())
}
