mod compositor;
mod controls;
mod daemon;
mod service;
mod wallpaper;
mod watch;

use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use wprs_core::{Config, dbus};

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
    let config = Config::load(&config_path)?;
    tracing::info!(path = %config_path.display(), "configuration loaded");
    let daemon =
        Arc::new(tokio::task::spawn_blocking(move || daemon::Daemon::new(config, config_path)).await??);
    let connection = zbus::connection::Builder::session()
        .name(dbus::BUS_NAME)
        .serve_at(dbus::OBJECT_PATH, service::Service::new(daemon.clone()))
        .build()
        .await
        .with_context(|| {
            format!("cannot own {} on the session bus (is another wprsd running?)", dbus::BUS_NAME)
        })?;
    tracing::info!("serving {} on the session bus", dbus::BUS_NAME);
    tokio::spawn(watch::battery(daemon.clone()));
    tokio::spawn(watch::screen_lock(connection.clone(), daemon.clone()));
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    tracing::info!("shutting down");
    Ok(())
}
